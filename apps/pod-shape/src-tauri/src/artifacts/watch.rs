//! Watches the hook output with the `notify` crate (HANDOFF §11.4.1): `<root>` recursively and the
//! log's own folder (`~/.claude/`) non-recursively, filtered to the log file. Events are debounced and
//! each burst runs one `sync`. A full sync runs first at start. When the folder or the log is
//! missing, a 30 s timer checks again, so the banner clears once the hook appears.

use super::paths::HookPaths;
use super::sync::{sync, SyncReport};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use rusqlite::Connection;
use serde::Serialize;
use std::path::Path;
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};

const DEBOUNCE: Duration = Duration::from_millis(500);
const RECHECK: Duration = Duration::from_secs(30);

/// What the banner and the "Last hook write" line show.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HookStatus {
    pub folder_found: bool,
    pub log_found: bool,
    /// Unix seconds of the last hook write: the log's mtime, else the newest artifact folder's mtime.
    pub last_write: Option<i64>,
    pub bad_lines: usize,
}

fn mtime(p: &Path) -> Option<i64> {
    std::fs::metadata(p).ok()?.modified().ok()?.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs() as i64)
}

/// True when a watcher event can mean new hook output: a rescan notice (events were lost: FSEvents
/// MustScanSubDirs, an inotify queue overflow; they come as `Other` with no paths, review round 2 N1),
/// or a change (create, modify, remove, rename) under the artifact folder or to the log file. Access
/// events (open, read, close without a write) and other unknown ones are ignored, because the app's own
/// reads cause them (review F4).
pub fn relevant(ev: &Event, root: &Path, log_name: Option<&std::ffi::OsStr>) -> bool {
    if ev.need_rescan() {
        return true;
    }
    if matches!(ev.kind, EventKind::Access(_) | EventKind::Other) {
        return false;
    }
    ev.paths.iter().any(|p| p.starts_with(root) || (log_name.is_some() && p.file_name() == log_name))
}

pub fn status(paths: &HookPaths, bad_lines: usize) -> HookStatus {
    let folder_found = paths.root.is_dir();
    let log_found = paths.log.is_file();
    let last_write = mtime(&paths.log).or_else(|| {
        std::fs::read_dir(&paths.root).ok()?.flatten().filter_map(|e| mtime(&e.path())).max()
    });
    HookStatus { folder_found, log_found, last_write, bad_lines }
}

/// Starts the watcher thread. `on_sync` runs after every sync (the app emits `artifacts-changed`).
pub fn start(db: Arc<Mutex<Connection>>, paths: HookPaths, on_sync: impl Fn(SyncReport) + Send + 'static) {
    std::thread::Builder::new()
        .name("artifact-hook-watch".into())
        .spawn(move || run(db, paths, on_sync))
        .expect("start the artifact hook watcher");
}

fn run(db: Arc<Mutex<Connection>>, paths: HookPaths, on_sync: impl Fn(SyncReport)) {
    let (tx, rx) = channel::<()>();
    let log_name = paths.log.file_name().map(|n| n.to_os_string());
    let log_dir = paths.log.parent().map(Path::to_path_buf);
    let root = paths.root.clone();
    let make_watcher = {
        let tx = tx.clone();
        move || -> Option<RecommendedWatcher> {
            let tx = tx.clone();
            let root_c = root.clone();
            let log_name = log_name.clone();
            let handler = move |res: notify::Result<Event>| {
                if res.is_ok_and(|ev| relevant(&ev, &root_c, log_name.as_deref())) {
                    let _ = tx.send(());
                }
            };
            // Symlinks under the folder are not followed (review F9) where the backend supports it (inotify).
            // FSEvents (macOS) ignores this setting, so the real control is the guard in paths.rs, which
            // rejects a symlinked folder or file whatever the watcher reports.
            let mut w = RecommendedWatcher::new(handler, Config::default().with_follow_symlinks(false)).ok()?;
            let mut any = false;
            if root.is_dir() {
                any |= w.watch(&root, RecursiveMode::Recursive).is_ok();
            }
            if let Some(d) = log_dir.as_ref().filter(|d| d.is_dir()) {
                any |= w.watch(d, RecursiveMode::NonRecursive).is_ok();
            }
            any.then_some(w)
        }
    };

    let mut watching = (paths.root.is_dir(), paths.log.is_file());
    let mut watcher = make_watcher();
    on_sync(sync(&db, &paths));
    loop {
        match rx.recv_timeout(RECHECK) {
            Ok(()) => {
                // Debounce: wait until the burst is quiet.
                while rx.recv_timeout(DEBOUNCE).is_ok() {}
                on_sync(sync(&db, &paths));
            }
            Err(RecvTimeoutError::Timeout) => {
                let now = (paths.root.is_dir(), paths.log.is_file());
                if watcher.is_none() && (now.0 || now.1) {
                    // The watcher could not start (for example no inotify watches left): poll instead.
                    watcher = make_watcher();
                    on_sync(sync(&db, &paths));
                } else if now != watching {
                    // The folder or the log appeared or went away: watch again and sync.
                    watching = now;
                    drop(watcher.take());
                    watcher = make_watcher();
                    on_sync(sync(&db, &paths));
                }
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// Unix seconds now.
#[cfg(test)]
fn now() -> i64 {
    std::time::SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifacts::store;
    use crate::db;
    use std::fs;
    use std::io::Write;
    use std::sync::mpsc;

    #[test]
    fn only_change_events_count() {
        use notify::event::{AccessKind, AccessMode, CreateKind, ModifyKind, RemoveKind};
        let root = Path::new("/h/claude-artifacts");
        let log = Some(std::ffi::OsStr::new("artifact-log.tsv"));
        let ev = |kind, p: &str| Event { kind, paths: vec![p.into()], attrs: Default::default() };
        let file = "/h/claude-artifacts/A1/v1_a.html";
        // A stream as inotify gives it while sync reads the folder and the log: no change at all.
        for e in [
            ev(EventKind::Access(AccessKind::Open(AccessMode::Read)), file),
            ev(EventKind::Access(AccessKind::Close(AccessMode::Read)), file),
            ev(EventKind::Access(AccessKind::Read), "/h/.claude/artifact-log.tsv"),
            ev(EventKind::Other, file),
            ev(EventKind::Create(CreateKind::File), "/h/.claude/settings.json"),
        ] {
            assert!(!relevant(&e, root, log), "{e:?}");
        }
        // Lost events: a rescan notice with no paths must trigger a sync (round 2 N1).
        let rescan = Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan);
        assert!(relevant(&rescan, root, log));
        for e in [
            ev(EventKind::Create(CreateKind::File), file),
            ev(EventKind::Modify(ModifyKind::Any), "/h/.claude/artifact-log.tsv"),
            ev(EventKind::Remove(RemoveKind::File), file),
            ev(EventKind::Access(AccessKind::Close(AccessMode::Write)), file),
        ] {
            let want = !matches!(e.kind, EventKind::Access(_));
            assert_eq!(relevant(&e, root, log), want, "{e:?}");
        }
    }

    #[test]
    fn status_reports_missing_parts() {
        let dir = tempfile::tempdir().unwrap();
        let paths = HookPaths { root: dir.path().join("claude-artifacts"), log: dir.path().join(".claude/artifact-log.tsv") };
        let s = status(&paths, 0);
        assert_eq!((s.folder_found, s.log_found, s.last_write), (false, false, None));
        fs::create_dir_all(&paths.root).unwrap();
        fs::create_dir_all(paths.log.parent().unwrap()).unwrap();
        fs::write(&paths.log, "").unwrap();
        let s = status(&paths, 2);
        assert!(s.folder_found && s.log_found);
        assert!(s.last_write.unwrap() >= now() - 60);
        assert_eq!(s.bad_lines, 2);
    }

    #[test]
    fn the_hook_sequence_tmp_mv_then_log_gives_one_version_with_the_log_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let paths = HookPaths { root: dir.path().join("claude-artifacts"), log: dir.path().join(".claude/artifact-log.tsv") };
        fs::create_dir_all(&paths.root).unwrap();
        fs::create_dir_all(paths.log.parent().unwrap()).unwrap();
        fs::write(&paths.log, "").unwrap();
        let db = Arc::new(Mutex::new(db::open(&dir.path().join("t.db")).unwrap()));
        let (tx, rx) = mpsc::channel();
        start(db.clone(), paths.clone(), move |r| {
            let _ = tx.send(r);
        });
        rx.recv_timeout(Duration::from_secs(10)).expect("the start sync");

        // What the hook does: mkdir, cp to .tmp, mv to the final name, then append the log line.
        let folder = paths.root.join("W1");
        fs::create_dir_all(&folder).unwrap();
        let file = folder.join("20261122T111340Z_proto.html");
        fs::write(folder.join("20261122T111340Z_proto.html.tmp"), "<p>w</p>").unwrap();
        fs::rename(folder.join("20261122T111340Z_proto.html.tmp"), &file).unwrap();
        let line = format!(
            "2026-11-22T11:13:40Z\thttps://claude.ai/artifact/W1\tWatched Page\t{}\t/u/code/pb-bot\t{}\n",
            folder.display(),
            file.display()
        );
        fs::OpenOptions::new().append(true).open(&paths.log).unwrap().write_all(line.as_bytes()).unwrap();

        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        let done = |db: &Mutex<Connection>| {
            let inbox = store::list_inbox(&db.lock().unwrap()).unwrap();
            inbox.len() == 1 && inbox[0].title == "Watched Page"
        };
        while !done(&db) && std::time::Instant::now() < deadline {
            let _ = rx.recv_timeout(Duration::from_millis(500));
        }
        let inbox = store::list_inbox(&db.lock().unwrap()).unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].title, "Watched Page");
        assert_eq!(inbox[0].cwd.as_deref(), Some("/u/code/pb-bot"));
        assert_eq!(inbox[0].versions, 1, "the .tmp is never a version");
    }
}

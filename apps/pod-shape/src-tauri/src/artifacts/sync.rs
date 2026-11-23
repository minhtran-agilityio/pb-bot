//! One pass over the hook output (HANDOFF §11.4): new log lines first, then a scan of the artifact
//! folder, which is the source of truth (§11.3). Running it again imports nothing new: versions
//! are deduped by hash and unchanged files are skipped by size and mtime.

use super::log::{parse_line, LogEntry, MAX_LINE_BYTES};
use super::paths::{artifact_dir, guard, is_html_name, read_guarded, valid_id, GuardedFile, HookPaths, Rejected};
use super::store::{self, HookMeta};
use rusqlite::Connection;
use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// What one sync did. `bad_lines` counts the bad log lines read since the app started (kept in
/// memory; it starts again at 0 when the log is read again from the start).
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub new_versions: usize,
    pub bad_lines: usize,
    pub skipped_files: usize,
}

const OFFSET: &str = "log_offset";
const LAST_LINE: &str = "log_last_line";
/// How much of the last line read is kept to recognise the log next time.
const LAST_LINE_TAIL: usize = 4096;

/// One sync at a time (watcher, start-up and `rescan_hook` share it).
static SYNC: Mutex<()> = Mutex::new(());
/// Bad lines per log file since the app started, in memory. Its own lock, held only to read or write
/// the number, so `hook_status` never waits for a running sync (review F3).
static BAD: Mutex<BTreeMap<PathBuf, usize>> = Mutex::new(BTreeMap::new());
/// The last bytes of the last line read, hex, to recognise the log next time (review F10: bytes, not
/// lossy text).
const LAST_TAIL_HEX: &str = "log_last_tail_hex";

/// Bad lines read from this log since the app started.
pub fn bad_lines(paths: &HookPaths) -> usize {
    BAD.lock().unwrap_or_else(|p| p.into_inner()).get(&paths.log).copied().unwrap_or(0)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    let b = s.as_bytes();
    if !b.len().is_multiple_of(2) {
        return None;
    }
    b.chunks(2).map(|p| std::str::from_utf8(p).ok().and_then(|h| u8::from_str_radix(h, 16).ok())).collect()
}

fn lock(db: &Mutex<Connection>) -> std::sync::MutexGuard<'_, Connection> {
    db.lock().unwrap_or_else(|p| p.into_inner())
}

/// `1970-01-01T00:00:00Z` form of a unix time (UTC), without a date crate.
pub fn iso_utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// One pass: new log lines (metadata only), then the folder scan (all HTML imports).
pub fn sync(db: &Mutex<Connection>, paths: &HookPaths) -> SyncReport {
    let _one = SYNC.lock().unwrap_or_else(|p| p.into_inner());
    let mut bad = bad_lines(paths);
    let mut report = SyncReport::default();
    read_log(db, paths, &mut bad);
    BAD.lock().unwrap_or_else(|p| p.into_inner()).insert(paths.log.clone(), bad);
    scan_folder(db, paths, &mut report);
    report.bad_lines = bad;
    report
}

/// The guarded `.html` files of one artifact folder, oldest first (mtime, then name).
/// Files already handled, or rejected for a reason that holds until they change (size, mtime), are
/// skipped without another check (review F10).
fn folder_files(db: &Mutex<Connection>, root: &Path, id: &str, report: &mut SyncReport) -> Vec<GuardedFile> {
    let Ok(dir) = artifact_dir(root, id) else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut files = Vec::new();
    for name in entries.flatten().filter_map(|e| e.file_name().to_str().map(str::to_string)).filter(|n| is_html_name(n)) {
        let path = dir.join(&name);
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        let (key, size, mtime) = (path.to_string_lossy().into_owned(), meta.len(), file_mtime(&meta));
        if store::file_seen(&lock(db), &key, size, mtime).unwrap_or(false) {
            continue;
        }
        match guard(root, id, &name) {
            Ok(g) => files.push(g),
            Err(why) => {
                eprintln!("shape-board: skipped hook file {id}/{name}: {why:?}");
                report.skipped_files += 1;
                if why.is_permanent() {
                    let _ = store::mark_file(&lock(db), &key, size, mtime);
                }
            }
        }
    }
    files.sort_by(|a, b| (a.mtime, &a.path).cmp(&(b.mtime, &b.path)));
    files
}

/// Reads a guarded file and stores it as a version of `artifact_id`; records the file as handled.
fn import_file(db: &Mutex<Connection>, artifact_id: i64, f: &GuardedFile, at: Option<&str>, report: &mut SyncReport) {
    let html = match read_guarded(f) {
        Ok(h) => h,
        Err(why) => {
            // `Changed` and `Missing` are tried again on the next sync; the others are recorded.
            if why != Rejected::Changed {
                eprintln!("shape-board: skipped hook file {:?}: {why:?}", f.path);
            }
            report.skipped_files += 1;
            if why.is_permanent() {
                let _ = store::mark_file(&lock(db), &f.path.to_string_lossy(), f.size, f.mtime);
            }
            return;
        }
    };
    let path = f.path.to_string_lossy();
    let conn = lock(db);
    match store::add_version_at(&conn, artifact_id, &html, "hook", Some(&path), at) {
        Ok(Some(_)) => report.new_versions += 1,
        Ok(None) => {}
        Err(e) => {
            eprintln!("shape-board: could not store {path}: {e}");
            return;
        }
    }
    let _ = store::mark_file(&conn, &path, f.size, f.mtime);
}

/// A log line gives metadata only (HANDOFF §11.3: the folder is the source of truth). The HTML of
/// every artifact is imported by the folder scan in file order, so version numbers follow time.
fn handle_entry(db: &Mutex<Connection>, paths: &HookPaths, e: &LogEntry) {
    let expected = paths.root.join(&e.id);
    if Path::new(&e.folder) != expected {
        // Column 4 is never used as a path; a different value is only worth a note.
        eprintln!("shape-board: log folder for {} is not {:?}; using the latter", e.id, expected);
    }
    let conn = lock(db);
    let meta = HookMeta {
        claude_id: &e.id,
        link: Some(&e.link),
        title: &e.title,
        cwd: e.cwd.as_deref(),
        published_at: Some(&e.published_at),
    };
    if let Err(err) = store::upsert_hook_artifact(&conn, &meta) {
        eprintln!("shape-board: could not store artifact {}: {err}", e.id);
    }
    // The line's time belongs to the version of its own file (column 6), also when that file came first.
    if let Some(file) = &e.file {
        if let Err(err) = store::set_file_time(&conn, file, &e.published_at) {
            eprintln!("shape-board: could not store the time of {file}: {err}");
        }
    }
}

/// True when the bytes right before `offset` are `tail` + newline: the log is the one read before.
fn same_log(file: &mut File, offset: u64, tail: &[u8]) -> bool {
    // With no stored tail only the newline would be compared, which every log matches: read again.
    if tail.is_empty() {
        return false;
    }
    let mut want = tail.to_vec();
    want.push(b'\n');
    let n = want.len() as u64;
    let mut buf = vec![0; want.len()];
    offset > 0 && n <= offset && file.seek(SeekFrom::Start(offset - n)).is_ok() && file.read_exact(&mut buf).is_ok() && buf == want
}

fn file_mtime(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn tail_of(line: &[u8]) -> &[u8] {
    &line[line.len().saturating_sub(LAST_LINE_TAIL)..]
}

fn read_log(db: &Mutex<Connection>, paths: &HookPaths, bad: &mut usize) {
    // O_NONBLOCK: a FIFO at the log path must not block the open (review F3); fstat then refuses it.
    let Ok(mut file) = OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(&paths.log) else { return };
    let Ok(meta) = file.metadata() else { return };
    if !meta.is_file() {
        return;
    }
    let size = meta.len();
    let (mut offset, last) = {
        let conn = lock(db);
        let off = store::get_state(&conn, OFFSET).ok().flatten().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
        // A state written before the tail was kept as bytes has only the text line: use its bytes, so the
        // first sync after the upgrade does not read the whole log again (round 2 nit).
        let tail = store::get_state(&conn, LAST_TAIL_HEX)
            .ok()
            .flatten()
            .and_then(|h| unhex(&h))
            .or_else(|| store::get_state(&conn, LAST_LINE).ok().flatten().map(|t| tail_of(t.as_bytes()).to_vec()))
            .unwrap_or_default();
        (off, tail)
    };
    // The log was replaced or cut when it is shorter than the offset or the line before the offset
    // is not the one stored. Then read it again from the start; the dedupe keeps that safe.
    if offset > size || (offset > 0 && !same_log(&mut file, offset, &last)) {
        offset = 0;
        *bad = 0;
    }
    if file.seek(SeekFrom::Start(offset)).is_err() {
        return;
    }
    let mut reader = BufReader::new(file);
    let mut pos = offset;
    let mut last_tail: Option<Vec<u8>> = None;
    loop {
        // Every read is bounded: at most one line of MAX_LINE_BYTES plus its newline.
        let mut line = Vec::new();
        let Ok(n) = (&mut reader).take(MAX_LINE_BYTES as u64 + 1).read_until(b'\n', &mut line) else { break };
        if n == 0 {
            break;
        }
        if line.last() == Some(&b'\n') {
            pos += n as u64;
            let text = String::from_utf8_lossy(&line[..line.len() - 1]);
            if !text.trim().is_empty() {
                match parse_line(&text) {
                    Ok(entry) => handle_entry(db, paths, &entry),
                    Err(why) => {
                        *bad += 1;
                        eprintln!("shape-board: skipped a bad artifact log line ending at byte {pos}: {why:?}");
                    }
                }
            }
            last_tail = Some(tail_of(&line[..line.len() - 1]).to_vec());
            continue;
        }
        if n <= MAX_LINE_BYTES {
            // A half-written last line: wait for the next sync.
            break;
        }
        // An over-long line: skip to its newline in bounded steps. If the log ends first, wait.
        let mut skipped = n as u64;
        let mut tail = line;
        let done = loop {
            let mut chunk = Vec::new();
            match (&mut reader).take(MAX_LINE_BYTES as u64).read_until(b'\n', &mut chunk) {
                Ok(0) | Err(_) => break false,
                Ok(m) => {
                    skipped += m as u64;
                    let ends = chunk.last() == Some(&b'\n');
                    tail = chunk;
                    if ends {
                        break true;
                    }
                }
            }
        };
        if !done {
            break;
        }
        pos += skipped;
        *bad += 1;
        eprintln!("shape-board: skipped an over-long artifact log line ending at byte {pos}");
        last_tail = Some(tail_of(&tail[..tail.len() - 1]).to_vec());
    }
    if let Some(t) = last_tail {
        let conn = lock(db);
        let _ = store::set_state(&conn, OFFSET, &pos.to_string());
        // The last line as text (HANDOFF §11.4.1) and its tail as bytes for the next comparison.
        let _ = store::set_state(&conn, LAST_LINE, &String::from_utf8_lossy(&t));
        let _ = store::set_state(&conn, LAST_TAIL_HEX, &hex(&t));
    }
}

fn scan_folder(db: &Mutex<Connection>, paths: &HookPaths, report: &mut SyncReport) {
    let Ok(entries) = std::fs::read_dir(&paths.root) else { return };
    let mut ids: Vec<String> = entries
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|n| valid_id(n))
        .collect();
    ids.sort();
    for id in ids {
        let files = folder_files(db, &paths.root, &id, report);
        let todo: Vec<&GuardedFile> = {
            let conn = lock(db);
            files
                .iter()
                .filter(|f| !store::file_seen(&conn, &f.path.to_string_lossy(), f.size, f.mtime).unwrap_or(false))
                .collect()
        };
        if todo.is_empty() {
            continue;
        }
        let artifact_id = {
            let conn = lock(db);
            // Known from the log, or a folder with no log line yet: the id is the title until the
            // log line arrives, and the newest file time is the publish time.
            let published = files.last().map(|f| iso_utc(f.mtime));
            let meta = HookMeta { claude_id: &id, link: None, title: &id, cwd: None, published_at: published.as_deref() };
            match store::upsert_hook_artifact(&conn, &meta) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("shape-board: could not store artifact {id}: {e}");
                    continue;
                }
            }
        };
        // A version counts from the publish time of the log line that names its file (column 6), else
        // from its file time (issue #100); never from the import time, and never later than a newer
        // version (`monotonic_times`).
        for f in todo {
            let at = store::file_time(&lock(db), &f.path.to_string_lossy()).ok().flatten().unwrap_or_else(|| iso_utc(f.mtime));
            import_file(db, artifact_id, f, Some(&at), report);
        }
        let _ = store::monotonic_times(&lock(db), artifact_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifacts::store::{add_to_project, list_artifacts, list_inbox, list_versions};
    use crate::db;
    use std::fs;
    use std::io::Write;
    use std::time::{Duration, SystemTime};

    pub(crate) struct Fx {
        pub _dir: tempfile::TempDir,
        pub paths: HookPaths,
        pub db: Mutex<Connection>,
    }

    pub(crate) fn fx() -> Fx {
        let dir = tempfile::tempdir().unwrap();
        let paths = HookPaths { root: dir.path().join("claude-artifacts"), log: dir.path().join(".claude/artifact-log.tsv") };
        fs::create_dir_all(&paths.root).unwrap();
        fs::create_dir_all(paths.log.parent().unwrap()).unwrap();
        let db = Mutex::new(db::open(&dir.path().join("t.db")).unwrap());
        Fx { _dir: dir, paths, db }
    }

    impl Fx {
        /// Writes a hook file with an explicit mtime (seconds after a fixed base), like the hook's mv.
        pub fn file_at(&self, id: &str, name: &str, body: &str, t: u64) -> String {
            let d = self.paths.root.join(id);
            fs::create_dir_all(&d).unwrap();
            let p = d.join(name);
            fs::write(&p, body).unwrap();
            let when = SystemTime::UNIX_EPOCH + Duration::from_secs(1_795_000_000 + t);
            fs::File::options().write(true).open(&p).unwrap().set_modified(when).unwrap();
            p.to_string_lossy().into_owned()
        }
        pub fn file(&self, id: &str, name: &str, body: &str) -> String {
            self.file_at(id, name, body, 0)
        }
        pub fn log(&self, line: &str) {
            let mut f = fs::OpenOptions::new().create(true).append(true).open(&self.paths.log).unwrap();
            f.write_all(line.as_bytes()).unwrap();
        }
        pub fn line6(&self, id: &str, title: &str, cwd: &str, file: &str) -> String {
            let folder = self.paths.root.join(id);
            format!("2026-11-22T11:13:40Z\thttps://claude.ai/artifact/{id}\t{title}\t{}\t{cwd}\t{file}\n", folder.display())
        }
        pub fn line4(&self, id: &str, title: &str) -> String {
            format!("2026-11-20T08:00:00Z\thttps://claude.ai/artifact/{id}\t{title}\t{}\n", self.paths.root.join(id).display())
        }
        fn sync(&self) -> SyncReport {
            sync(&self.db, &self.paths)
        }
        fn html_by_n(&self, artifact_id: i64) -> Vec<(i64, String)> {
            let c = self.db.lock().unwrap();
            let mut st = c.prepare("SELECT n, html FROM artifact_versions WHERE artifact_id = ?1 ORDER BY n").unwrap();
            st.query_map([artifact_id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(Result::unwrap).collect()
        }
        fn inbox(&self) -> Vec<crate::artifacts::store::InboxItem> {
            list_inbox(&self.db.lock().unwrap()).unwrap()
        }
    }

    #[test]
    fn iso_utc_formats_unix_time() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(1_795_346_020), "2026-11-22T11:13:40Z");
        assert_eq!(iso_utc(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn missing_folder_and_log_do_nothing() {
        let f = fx();
        fs::remove_dir_all(&f.paths.root).unwrap();
        assert_eq!(f.sync().new_versions, 0);
    }

    #[test]
    fn a_six_column_line_gives_title_cwd_and_time_and_a_second_sync_adds_nothing() {
        let f = fx();
        let file = f.file("Hook1", "20261122T111340Z_hook-test.html", "<h1>v1</h1>");
        f.log(&f.line6("Hook1", "Hook Test Page", "/u/code/pb-bot", &file));
        let r = f.sync();
        assert_eq!(r.new_versions, 1);
        let inbox = f.inbox();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].title, "Hook Test Page");
        assert_eq!(inbox[0].cwd.as_deref(), Some("/u/code/pb-bot"));
        assert_eq!(inbox[0].published_at.as_deref(), Some("2026-11-22T11:13:40Z"));
        assert_eq!(f.sync().new_versions, 0);
    }

    #[test]
    fn a_version_counts_from_the_hook_publish_time_not_the_import_time() {
        let f = fx();
        let file = f.file("Hook1", "20261122T111340Z_hook-test.html", "<h1>v1</h1>");
        f.log(&f.line6("Hook1", "Hook Test Page", "/u/code/pb-bot", &file));
        f.sync();
        let a = f.inbox()[0].id;
        let v = list_versions(&f.db.lock().unwrap(), a).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].created_at, "2026-11-22T11:13:40Z", "column 1 of the log, as the card shows");
    }

    #[test]
    fn a_rejected_newer_file_does_not_lend_its_time_to_an_older_version() {
        let f = fx();
        let max = super::super::paths::MAX_HTML_BYTES as usize;
        let l = |t: &str, file: &str| format!("{t}\thttps://claude.ai/artifact/Hook1\tT\t{}\t/c\t{file}\n", f.paths.root.join("Hook1").display());
        let v1 = f.file_at("Hook1", "1.html", "<p>1</p>", 1);
        let v2 = f.file_at("Hook1", "2.html", "<p>2</p>", 2);
        let d = f.paths.root.join("Hook1");
        let v3 = d.join("3.html").to_string_lossy().into_owned();
        fs::write(&v3, vec![b'a'; max + 1]).unwrap();
        f.log(&(l("2026-11-22T10:00:00Z", &v1) + &l("2026-11-22T11:00:00Z", &v2) + &l("2026-11-22T12:00:00Z", &v3)));
        f.sync();
        let a = f.inbox()[0].id;
        let v = list_versions(&f.db.lock().unwrap(), a).unwrap();
        let times: Vec<_> = v.iter().map(|x| (x.n, x.created_at.as_str())).collect();
        assert_eq!(times, vec![(2, "2026-11-22T11:00:00Z"), (1, "2026-11-22T10:00:00Z")]);
        assert_eq!(f.inbox()[0].published_at.as_deref(), Some("2026-11-22T12:00:00Z"), "the card keeps the newest line");
    }

    #[test]
    fn a_line_that_arrives_after_its_file_sets_that_versions_time() {
        let f = fx();
        let v1 = f.file_at("Hook1", "1.html", "<p>1</p>", 1);
        f.sync();
        let a = f.inbox()[0].id;
        assert!(list_versions(&f.db.lock().unwrap(), a).unwrap()[0].created_at.starts_with("2026-11-1"), "the file time first");
        f.log(&f.line6("Hook1", "T", "/c", &v1));
        f.sync();
        assert_eq!(list_versions(&f.db.lock().unwrap(), a).unwrap()[0].created_at, "2026-11-22T11:13:40Z");
    }

    #[test]
    fn files_without_a_line_use_their_own_time_and_the_order_never_goes_backwards() {
        let f = fx();
        let p1 = f.file_at("Hook1", "1.html", "<p>1</p>", 10);
        let p2 = f.file_at("Hook1", "2.html", "<p>2</p>", 20);
        // No line: each version has its own file time.
        f.sync();
        let a = f.inbox()[0].id;
        let t = |n: usize| list_versions(&f.db.lock().unwrap(), a).unwrap()[n].created_at.clone();
        assert_eq!(t(1), iso_utc(1_795_000_010));
        assert_eq!(t(0), iso_utc(1_795_000_020));
        // A line for v1 whose clock is far ahead: v1 is capped at v2's time.
        f.log(&format!("2030-01-01T00:00:00Z\thttps://claude.ai/artifact/Hook1\tT\t{}\t/c\t{p1}\n", f.paths.root.join("Hook1").display()));
        f.sync();
        assert!(t(1) <= t(0), "{} then {}", t(1), t(0));
        let _ = p2;
    }

    #[test]
    fn a_hook_file_with_no_log_line_uses_its_file_time() {
        let f = fx();
        f.file_at("NoLog1", "a.html", "<p>a</p>", 10);
        f.sync();
        let a = f.inbox()[0].id;
        let v = list_versions(&f.db.lock().unwrap(), a).unwrap();
        assert!(v[0].created_at.starts_with("20") && v[0].created_at != "2026-11-22T11:13:40Z");
        assert_eq!(v[0].created_at, f.inbox()[0].published_at.clone().unwrap());
    }

    #[test]
    fn a_first_scan_with_four_column_lines_numbers_versions_in_time_order() {
        let f = fx();
        // Names sort the other way round from time, so only mtime can give this order.
        f.file_at("Old1", "z_first.html", "<p>first</p>", 10);
        f.file_at("Old1", "m_second.html", "<p>second</p>", 20);
        f.file_at("Old1", "a_third.html", "<p>third</p>", 30);
        f.log(&f.line4("Old1", "Old page"));
        assert_eq!(f.sync().new_versions, 3);
        let a = f.inbox()[0].clone();
        assert_eq!(a.title, "Old page");
        let got: Vec<String> = f.html_by_n(a.id).into_iter().map(|(_, h)| h).collect();
        assert_eq!(got, vec!["<p>first</p>", "<p>second</p>", "<p>third</p>"]);
    }

    #[test]
    fn a_revert_a_b_a_from_the_hook_is_three_versions() {
        let f = fx();
        f.file_at("Rev1", "1.html", "A", 1);
        f.file_at("Rev1", "2.html", "B", 2);
        f.file_at("Rev1", "3.html", "A", 3);
        assert_eq!(f.sync().new_versions, 3);
        // An identical copy of the latest is not a new version.
        f.file_at("Rev1", "4.html", "A", 4);
        assert_eq!(f.sync().new_versions, 0);
    }

    #[test]
    fn bad_lines_are_skipped_and_counted_in_memory_and_good_ones_still_work() {
        let f = fx();
        let file = f.file("Good1", "v_g.html", "<p>g</p>");
        f.log("not a log line\n");
        f.log("2026-11-20T08:00:00Z\thttps://evil.example/artifact/x\tt\t/x\n");
        f.log("2026-11-20T08:00:00Z\thttps://claude.ai/artifact/..\tt\t/x\n");
        f.log("2026-11-20T08:00:00Z\thttps://claude.ai/artifact/Good1?x=1\tt\t/x\n");
        f.log("2026-11-20T08:00:00Z\thttps://claude.ai/artifact/Good1/\tt\t/x\n");
        f.log(&f.line6("Good1", "Good", "", &file));
        let r = f.sync();
        assert_eq!((r.new_versions, r.bad_lines), (1, 5));
        assert_eq!(bad_lines(&f.paths), 5);
        assert_eq!(f.inbox()[0].title, "Good");
        let n: i64 = f.db.lock().unwrap().query_row("SELECT COUNT(*) FROM hook_state WHERE key = 'bad_lines'", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0, "the count is not stored in SQLite");
    }

    #[test]
    fn the_bad_line_count_counts_each_bad_line_once() {
        let f = fx();
        f.log("junk 1\n");
        f.log("junk 2\n");
        assert_eq!(f.sync().bad_lines, 2);
        assert_eq!(f.sync().bad_lines, 2);
        f.log("junk 3\n");
        assert_eq!(f.sync().bad_lines, 3);
        // A replaced log is read again from the start, and the count starts again.
        fs::write(&f.paths.log, "junk\n").unwrap();
        assert_eq!(f.sync().bad_lines, 1);
    }

    #[test]
    fn log_columns_cannot_make_the_app_read_outside_the_folder() {
        let f = fx();
        let secret = f.paths.root.parent().unwrap().join("secret.html");
        fs::write(&secret, "<p>secret</p>").unwrap();
        f.log(&format!(
            "2026-11-20T08:00:00Z\thttps://claude.ai/artifact/Evil1\tEvil\t{}\t/u\t{}\n",
            f.paths.root.parent().unwrap().display(),
            f.paths.root.join("Evil1/../../secret.html").display()
        ));
        f.sync();
        let n: i64 = f.db.lock().unwrap().query_row("SELECT COUNT(*) FROM artifact_versions WHERE html LIKE '%secret%'", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
        assert!(f.inbox().is_empty(), "no folder, no version, nothing offered");
    }

    #[test]
    fn folder_scan_without_a_log_imports_html_only_and_skips_junk() {
        let f = fx();
        f.file_at("Scan1", "1_a.html", "<p>a</p>", 1);
        f.file_at("Scan1", "2_b.html", "<p>b</p>", 2);
        f.file_at("Scan1", "3_same.html", "<p>b</p>", 3);
        f.file("Scan1", "4_c.html.tmp", "<p>tmp</p>");
        f.file("Scan1", "notes.txt", "x");
        f.file("Scan1", "big.html", &"a".repeat(super::super::paths::MAX_HTML_BYTES as usize + 1));
        fs::create_dir_all(f.paths.root.join("bad id")).unwrap();
        fs::write(f.paths.root.join("loose.html"), "<p>not in an artifact folder</p>").unwrap();
        let r = f.sync();
        assert_eq!(r.new_versions, 2, "a and b; a copy of the latest is not a version");
        assert_eq!(r.skipped_files, 1, "the 5 MiB+1 file");
        let inbox = f.inbox();
        assert_eq!((inbox.len(), inbox[0].title.as_str()), (1, "Scan1"));
        assert_eq!(f.sync().new_versions, 0);
    }

    #[test]
    fn a_symlinked_id_folder_and_a_fifo_are_skipped_without_hanging() {
        let f = fx();
        let outside = f.paths.root.parent().unwrap().join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("x.html"), "<p>outside</p>").unwrap();
        std::os::unix::fs::symlink(&outside, f.paths.root.join("Linked1")).unwrap();
        fs::create_dir_all(f.paths.root.join("Fifo1")).unwrap();
        let c = std::ffi::CString::new(f.paths.root.join("Fifo1/p.html").to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        let r = f.sync();
        assert_eq!(r.new_versions, 0);
        assert!(f.inbox().is_empty());
    }

    #[test]
    fn a_new_file_for_an_added_artifact_is_a_new_version_marked_updated() {
        let f = fx();
        let p = db::insert_project(&f.db.lock().unwrap(), "o/pb-bot", "pb-bot", "/u/code/pb-bot").unwrap();
        let v1 = f.file_at("Up1", "20261122T100000Z_p.html", "<p>1</p>", 1);
        f.log(&f.line6("Up1", "Proto", "/u/code/pb-bot", &v1));
        f.sync();
        let a = f.inbox()[0].clone();
        assert_eq!(a.suggested_project_id, Some(p.id));
        add_to_project(&f.db.lock().unwrap(), a.id, p.id).unwrap();
        assert!(!list_artifacts(&f.db.lock().unwrap(), p.id).unwrap()[0].updated);

        let v2 = f.file_at("Up1", "20261123T100000Z_p.html", "<p>2</p>", 2);
        f.log(&f.line6("Up1", "Proto", "/u/code/pb-bot", &v2));
        assert_eq!(f.sync().new_versions, 1);
        let s = list_artifacts(&f.db.lock().unwrap(), p.id).unwrap()[0].clone();
        assert_eq!((s.versions, s.updated), (2, true));
        assert!(f.inbox().is_empty(), "it stays in its project");
        assert_eq!(list_versions(&f.db.lock().unwrap(), a.id).unwrap().len(), 2);
    }

    #[test]
    fn half_written_lines_wait_and_a_replaced_log_is_reread_without_duplicates() {
        let f = fx();
        let a = f.file("Inc1", "a.html", "<p>a</p>");
        let b = f.file("Inc2", "b.html", "<p>b</p>");
        let line_b = f.line6("Inc2", "B", "", &b);
        f.log(&f.line6("Inc1", "A", "", &a));
        f.log(&line_b[..20]);
        f.sync();
        let titles = |f: &Fx| -> Vec<String> {
            let mut t: Vec<_> = f.inbox().into_iter().map(|i| i.title).collect();
            t.sort();
            t
        };
        assert_eq!(titles(&f), vec!["A", "Inc2"], "Inc2 is known from its folder, not yet from its line");
        f.log(&line_b[20..]);
        f.sync();
        assert_eq!(titles(&f), vec!["A", "B"]);

        fs::write(&f.paths.log, f.line6("Inc1", "A renamed", "", &a)).unwrap();
        assert_eq!(f.sync().new_versions, 0);
        assert_eq!(titles(&f), vec!["A renamed", "B"]);
        let total: i64 = f.db.lock().unwrap().query_row("SELECT COUNT(*) FROM artifact_versions", [], |r| r.get(0)).unwrap();
        assert_eq!(total, 2);
    }

    #[test]
    fn an_over_long_line_is_skipped_in_bounded_steps_and_waits_without_a_newline() {
        let f = fx();
        let a = f.file("Long1", "a.html", "<p>a</p>");
        f.log(&"x".repeat(MAX_LINE_BYTES * 3));
        let r1 = f.sync();
        let off1 = store::get_state(&f.db.lock().unwrap(), OFFSET).unwrap();
        assert_eq!(off1, None, "no newline yet: the offset does not move");
        f.log("\n");
        f.log(&f.line6("Long1", "After the long line", "", &a));
        let r2 = f.sync();
        assert_eq!((r1.bad_lines, r2.bad_lines), (0, 1));
        assert_eq!(f.inbox()[0].title, "After the long line");
    }

    #[test]
    fn the_last_line_read_is_stored_in_sqlite() {
        let f = fx();
        let a = f.file("St1", "a.html", "<p>a</p>");
        let line = f.line6("St1", "A", "", &a);
        f.log(&line);
        f.sync();
        let c = f.db.lock().unwrap();
        assert_eq!(store::get_state(&c, LAST_LINE).unwrap().unwrap(), line.trim_end_matches('\n'));
        assert_eq!(store::get_state(&c, OFFSET).unwrap().unwrap(), line.len().to_string());
    }

    #[test]
    fn a_fifo_at_the_log_path_neither_blocks_nor_counts() {
        let f = fx();
        let c = std::ffi::CString::new(f.paths.log.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        f.file("Fifo2", "a.html", "<p>a</p>");
        // On a thread with a deadline, so a regression fails the test instead of hanging it.
        let (tx, rx) = std::sync::mpsc::channel();
        let f = std::sync::Arc::new(f);
        let f2 = f.clone();
        std::thread::spawn(move || {
            let _ = tx.send(f2.sync());
        });
        let r = rx.recv_timeout(std::time::Duration::from_secs(5)).expect("sync must not block on a FIFO log");
        assert_eq!((r.new_versions, r.bad_lines), (1, 0));
        // hook_status reads the count without waiting for a sync.
        assert_eq!(bad_lines(&f.paths), 0);
    }

    #[test]
    fn rejected_files_are_recorded_and_not_counted_again() {
        let f = fx();
        f.file("Rej1", "big.html", &"a".repeat(super::super::paths::MAX_HTML_BYTES as usize + 1));
        assert_eq!(f.sync().skipped_files, 1);
        assert_eq!(f.sync().skipped_files, 0, "a permanent rejection is not checked again");
        // Once the file changes it is checked again.
        f.file_at("Rej1", "big.html", "<p>now small</p>", 5);
        assert_eq!(f.sync().new_versions, 1);
    }

    #[test]
    fn a_non_utf8_last_line_does_not_make_every_sync_reread_the_log() {
        let f = fx();
        f.log("junk \u{0}\n");
        let mut raw = fs::OpenOptions::new().append(true).open(&f.paths.log).unwrap();
        raw.write_all(&[b'x', 0xff, 0xfe, b'\n']).unwrap();
        assert_eq!(f.sync().bad_lines, 2);
        // A second sync reads nothing new: the count stays (a reread from 0 would reset and recount the
        // same 2, so also check the offset did not move back).
        let off = store::get_state(&f.db.lock().unwrap(), OFFSET).unwrap();
        assert_eq!(f.sync().bad_lines, 2);
        f.log("junk again\n");
        assert_eq!(f.sync().bad_lines, 3, "a reread from the start would give 3 only if it did not reset");
        assert_eq!(store::get_state(&f.db.lock().unwrap(), OFFSET).unwrap().map(|o| o > off.clone().unwrap()), Some(true));
    }

    #[test]
    fn a_state_with_only_the_text_line_still_recognises_its_log() {
        let f = fx();
        let a = f.file("Up2", "a.html", "<p>a</p>");
        let line = f.line6("Up2", "From the log", "", &a);
        f.log(&line);
        f.sync();
        // As an older state would look: no tail hex, only the text line. A marker shows a reread.
        let drop_hex = |f: &Fx| {
            let c = f.db.lock().unwrap();
            c.execute("DELETE FROM hook_state WHERE key = ?1", [LAST_TAIL_HEX]).unwrap();
            c.execute("UPDATE artifacts SET title = 'Marker' WHERE claude_id = 'Up2'", []).unwrap();
        };
        drop_hex(&f);
        f.log("junk\n");
        f.sync();
        assert_eq!(f.inbox()[0].title, "Marker", "the same log is not read again from the start");
        // A replaced log of the same shape (a newline right before the old offset) is noticed.
        drop_hex(&f);
        let offset: usize = store::get_state(&f.db.lock().unwrap(), OFFSET).unwrap().unwrap().parse().unwrap();
        let other = line.replace("From the log", "Replaced log");
        let mut body = other.clone();
        while body.len() < offset {
            body.push_str(&other);
        }
        fs::write(&f.paths.log, body).unwrap();
        f.sync();
        assert_eq!(f.inbox()[0].title, "Replaced log", "a different log is read from the start");
    }

    #[test]
    fn two_syncs_at_once_import_each_file_once() {
        let f = std::sync::Arc::new(fx());
        for i in 0..20 {
            f.file_at("Many1", &format!("{i:02}.html"), &format!("<p>{i}</p>"), i);
        }
        let hs: Vec<_> = (0..2)
            .map(|_| {
                let f = f.clone();
                std::thread::spawn(move || f.sync().new_versions)
            })
            .collect();
        let total: usize = hs.into_iter().map(|h| h.join().unwrap()).sum();
        assert_eq!(total, 20);
        let n: Vec<i64> = f.html_by_n(f.inbox()[0].id).into_iter().map(|(n, _)| n).collect();
        assert_eq!(n, (1..=20).collect::<Vec<_>>());
    }
}

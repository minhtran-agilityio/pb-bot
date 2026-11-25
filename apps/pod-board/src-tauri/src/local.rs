//! The Claude sessions of this machine, read from the files Claude Code itself keeps (issue #137, step 0 in the
//! README "Data and privacy"). Read only: no hook, no network, no write.
//!
//! - `~/.claude/sessions/<pid>.json`, one per running session: `status`, `waitingFor`, `bridgeSessionId` (the Remote
//!   Control id), `tmux` (`<tmux session>:@w.%p`, the tmux session is Pod Bay's `name`), `entrypoint`, `kind`, `name`,
//!   `statusUpdatedAt`, `cwd`, `sessionId`, `pid`. Claude deletes the file when the session exits.
//! - `~/.claude/projects/<cwd as a slug>/<sessionId>/subagents/agent-*.meta.json`, one per sub-agent: `agentType`,
//!   `description`, `model`, `spawnDepth` (1 = a child of the main session), `finished` (true once it ended). The
//!   sub-agent's transcript `agent-*.jsonl` next to it is never opened: only its file times are used (created = the
//!   start, last change = the end).
//!
//! Each field is copied by name into `Local`. `cwd`, `sessionId` and `pid` are used to find the files and to check
//! that the session still runs, and are then dropped: only the last folder name of `cwd` is kept, as the project.

use crate::model::{session_id, safe_name, Agent, Agents};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// At most this many session files are looked at in one poll (the newest first), at most this many live sessions
/// are kept, and each file is at most this big.
const MAX_LISTED: usize = 512;
const MAX_SESSIONS: usize = 64;
const MAX_FILE: u64 = 64 << 10;
/// At most this many sub-agent files of one session are read; at most this many rows reach the page.
const MAX_AGENT_FILES: usize = 400;
const MAX_AGENT_ROWS: usize = 40;
/// A sub-agent with no `finished` mark whose transcript has not changed for this long is not counted as running: it
/// was stopped without its mark (a crash, a killed session). A running one writes to its transcript all the time.
pub const STALE_MS: u64 = 15 * 60 * 1000;
const TEXT_CHARS: usize = 80;
/// A folder's listing is used again while the folder has not changed, for at most this long (issue #158). A listing
/// made less than `SETTLE` after the folder's last change is not used again: a file added in the same clock tick
/// leaves the folder time as it was.
const RELIST_MS: u64 = 60_000;
const SETTLE: std::time::Duration = std::time::Duration::from_secs(1);

/// What `read_with` keeps from one poll to the next, so a poll does not list and stat every file of an unchanged
/// folder, nor read a finished sub-agent again (a finished one does not change). Only what the last poll used is kept.
#[derive(Default)]
pub struct Cache {
    /// Folder -> (its change time, when it was listed, the files kept: the newest first, at most the cap).
    lists: HashMap<PathBuf, (SystemTime, SystemTime, Vec<PathBuf>)>,
    /// A finished sub-agent's `.meta.json` -> (its change time and size when read, its row). A file that changed
    /// since is read again.
    done: HashMap<PathBuf, (SystemTime, u64, Agent)>,
    used: HashSet<PathBuf>,
}

impl Cache {
    /// The files of `dir` whose name `keep` accepts, the newest first, at most `cap`: the last listing while the
    /// folder is unchanged and settled, else a new one.
    fn list(&mut self, dir: &Path, keep: fn(&str) -> bool, cap: usize, now: u64) -> Option<Vec<PathBuf>> {
        let changed = std::fs::metadata(dir).ok()?.modified().ok();
        self.used.insert(dir.to_path_buf());
        if let (Some(t), Some((at, listed, files))) = (changed, self.lists.get(dir)) {
            let settled = listed.duration_since(*at).map_or(false, |d| d >= SETTLE);
            if *at == t && settled && now.saturating_sub(ms(*listed)) < RELIST_MS {
                return Some(files.clone());
            }
        }
        let listed = SystemTime::now();
        let mut files: Vec<(SystemTime, PathBuf)> = std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .filter(|e| e.file_name().to_str().map_or(false, keep))
            .map(|e| (e.metadata().and_then(|m| m.modified()).unwrap_or(UNIX_EPOCH), e.path()))
            .collect();
        files.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        let files: Vec<PathBuf> = files.into_iter().take(cap).map(|f| f.1).collect();
        if let Some(t) = changed {
            self.lists.insert(dir.to_path_buf(), (t, listed, files.clone()));
        }
        Some(files)
    }

    /// Forget what this poll did not use (a session that ended, a sub-agent file that is gone).
    fn prune(&mut self) {
        let used = std::mem::take(&mut self.used);
        self.lists.retain(|k, _| used.contains(k));
        self.done.retain(|k, _| used.contains(k));
    }
}

/// One local session, with only the fields the board shows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Local {
    /// The tmux session it runs in (Pod Bay's `name` for the sessions Pod Bay runs), else empty.
    pub tmux: String,
    /// The Remote Control id (`bridgeSessionId`), when Remote Control is on and the id is well formed; else empty.
    pub rc_id: String,
    /// `entrypoint`: "cli" for every session seen on Linux and macOS (2026-11-25).
    pub entrypoint: String,
    /// `kind` is "interactive" (not a `claude -p` run).
    pub interactive: bool,
    /// `status`: "busy", "idle" or "waiting".
    pub status: String,
    /// `waitingFor` while it waits: "permission prompt" or "input needed" (seen).
    pub waiting_for: String,
    /// The session's name (`name`), else empty.
    pub title: String,
    /// The last folder name of `cwd`.
    pub project: String,
    /// `statusUpdatedAt` (ms since the epoch): since when it is in this state.
    pub since: i64,
    pub agents: Option<Agents>,
}

/// The real home folder (`$HOME`, no trailing `/`), when it is set and is not `/`.
fn home() -> Option<&'static str> {
    static HOME: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    HOME.get_or_init(|| std::env::var("HOME").ok().map(|h| h.trim_end_matches('/').to_string()).filter(|h| !h.is_empty()))
        .as_deref()
}

/// The login names of this user: the last folder of `$HOME`, `$USER` and `$LOGNAME`.
fn logins() -> &'static [String] {
    static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| {
        let mut names: Vec<String> = home().and_then(|h| h.rsplit('/').next()).map(str::to_string).into_iter().collect();
        names.extend(["USER", "LOGNAME"].iter().filter_map(|k| std::env::var(k).ok()));
        names.retain(|n| !n.is_empty() && !n.contains('/'));
        names.dedup();
        names
    })
}

/// One absolute path without a home folder or other folders: a path in a home folder (`$HOME`, `<any>/home/<user>`,
/// `/Users/<user>`, `/root`) becomes `~/…` below it; any other path of two or more folders keeps only its last part,
/// after `…/`. A one-part path (`/tmp`) stays.
fn fold_path(p: &str) -> String {
    if let Some(h) = home() {
        if let Some(rest) = p.strip_prefix(h).filter(|r| r.is_empty() || r.starts_with('/')) {
            return format!("~{rest}");
        }
    }
    let parts: Vec<&str> = p.split('/').filter(|x| !x.is_empty()).collect();
    if let Some(i) = parts.iter().position(|x| *x == "home" || *x == "Users").filter(|&i| i + 1 < parts.len()) {
        return std::iter::once("~").chain(parts[i + 2..].iter().copied()).collect::<Vec<_>>().join("/");
    }
    if parts.first() == Some(&"root") {
        return std::iter::once("~").chain(parts[1..].iter().copied()).collect::<Vec<_>>().join("/");
    }
    match parts.as_slice() {
        [last, ..] | [.., last] if logins().iter().any(|n| n == last) => "~".to_string(),
        [] | [_] => p.to_string(),
        [.., last] => format!("…/{last}"),
    }
}

/// The second pass over each word that holds a `/` (a path after `file://`, `host:` or a comma, which the first pass
/// does not see as a path): a whole folder name that comes after a `home` or `Users` folder, or that is a login name,
/// becomes `~`. Only whole folder names change (`src/device.rs` stays with the login `dev`); words without a `/` are
/// left as written.
fn scrub(s: &str) -> String {
    const CLOSE: &str = "\"'`)]}>,;:";
    let word = |w: &str| -> String {
        if !w.contains('/') {
            return w.to_string();
        }
        let mut prev = "";
        let mut out: Vec<String> = Vec::new();
        for seg in w.split('/') {
            let (core, tail) = seg.split_at(seg.find(|c: char| CLOSE.contains(c)).unwrap_or(seg.len()));
            let hide = !core.is_empty() && core != "~" && (prev == "home" || prev == "Users" || logins().iter().any(|n| n == core));
            out.push(if hide { format!("~{tail}") } else { seg.to_string() });
            prev = core;
        }
        out.join("/")
    };
    let mut out = String::with_capacity(s.len());
    let mut start = 0;
    for (i, c) in s.char_indices() {
        if c.is_whitespace() {
            out.push_str(&word(&s[start..i]));
            out.push(c);
            start = i + c.len_utf8();
        }
    }
    out.push_str(&word(&s[start..]));
    out
}

/// Every absolute path in a text loses its folders (`fold_path`), then home folders and login names in any other
/// word with a `/` go (`scrub`), so a model-written text (a sub-agent's task)
/// cannot carry a full path or the login name in a home path to the page. A path starts with `/` at the start of
/// the text or after a space or an opening mark, and ends at a space, a closing mark, `,` or `;`. A web address
/// (`https://…`) is not a path.
fn home_free(s: &str) -> String {
    let opens = |c: char| c.is_whitespace() || "\"'`([{<=".contains(c);
    let ends = |c: char| c.is_whitespace() || "\"'`)]}>,;".contains(c);
    let mut out = String::with_capacity(s.len());
    let mut prev: Option<char> = None;
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        if c == '/' && prev.map_or(true, opens) {
            let end = rest.find(ends).unwrap_or(rest.len());
            out.push_str(&fold_path(&rest[..end]));
            prev = rest[..end].chars().last();
            rest = &rest[end..];
            continue;
        }
        out.push(c);
        prev = Some(c);
        rest = &rest[c.len_utf8()..];
    }
    scrub(&out)
}

/// Characters that are dropped from a text from a file: control, text-direction and zero-width characters (they
/// could disguise a notification or make one name look like another).
fn hidden(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200B}'..='\u{200F}' | '\u{FEFF}' | '\u{061C}' | '\u{2060}')
}

/// Shorten a text from a file, without hidden characters and home paths.
fn cut(s: &str, n: usize) -> String {
    // Hidden characters go first: one before a `/` would hide a path from `home_free`.
    let shown: String = s.chars().filter(|&c| !hidden(c)).collect();
    let clean = home_free(&shown);
    let s = clean.trim();
    if s.chars().count() <= n {
        return s.to_string();
    }
    let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn ms(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// A small JSON file: a plain file (not a link, not a pipe), read once and at most `cap` bytes.
fn read_json(path: &Path, cap: u64) -> Option<Value> {
    use std::io::Read;
    let mut open = std::fs::OpenOptions::new();
    open.read(true);
    // No link is followed and a pipe cannot block the poll: open does not wait, and a non-file is refused below.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        open.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(not(unix))]
    if !std::fs::symlink_metadata(path).ok()?.file_type().is_file() {
        return None;
    }
    let file = open.open(path).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut text = String::new();
    file.take(cap + 1).read_to_string(&mut text).ok()?;
    if text.len() as u64 > cap {
        return None;
    }
    serde_json::from_str(&text).ok()
}

/// The last folder name of a path, as the project. A home folder (`$HOME`, `<any>/home/<user>`, `/Users/<user>`,
/// `/root`) or a folder named like the login is `~`, so the login name never shows.
fn project_of(cwd: &str) -> String {
    let path = cwd.trim_end_matches('/');
    let (parent, last) = path.rsplit_once('/').unwrap_or(("", path));
    let in_homes = matches!(parent.rsplit('/').next(), Some("home" | "Users"));
    if last.is_empty() || path == "/root" || in_homes || home() == Some(path) || logins().iter().any(|n| n == last) {
        return "~".to_string();
    }
    cut(last, TEXT_CHARS)
}

/// Claude's folder name for a working folder under `~/.claude/projects`: every character other than a letter or a
/// digit becomes `-` (seen: `/home/u/my.repo` -> `-home-u-my-repo`).
fn slug(cwd: &str) -> String {
    cwd.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

/// A session id that is safe as one folder name: hex digits and `-` only.
fn safe_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// Whether process `pid` still runs and is the one the file was written by (a session file outlives its session only
/// when it was killed hard, and then its pid can be reused). `proc_start` is the file's `procStart`: on Linux the
/// process start time in clock ticks, field 22 of `/proc/<pid>/stat` (seen 2026-11-25); a mismatch is another process.
/// On macOS its format was not seen, so there only the pid is checked (README, Limits).
pub fn alive(pid: i64, proc_start: &str) -> bool {
    #[cfg(unix)]
    {
        if pid <= 0 || pid > i32::MAX as i64 {
            return false;
        }
        // Signal 0 checks the process without touching it. EPERM: it runs under another user.
        let r = unsafe { libc::kill(pid as libc::pid_t, 0) };
        if !(r == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)) {
            return false;
        }
        #[cfg(target_os = "linux")]
        if !proc_start.is_empty() {
            if let Some(now) = linux_start(pid) {
                return now == proc_start;
            }
        }
        let _ = proc_start;
        true
    }
    #[cfg(not(unix))]
    {
        let _ = (pid, proc_start);
        true
    }
}

/// Field 22 (start time) of `/proc/<pid>/stat`. The name in field 2 can hold spaces, so count from its `)`.
#[cfg(target_os = "linux")]
fn linux_start(pid: i64) -> Option<String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rest = &stat[stat.rfind(')')? + 1..];
    rest.split_whitespace().nth(19).map(str::to_string)
}

/// The sub-agents of one session, from its `subagents` folder. None when it has none.
fn agents_in(dir: &Path, now: u64, cache: &mut Cache) -> Option<Agents> {
    // The newest files first, so a long session's running sub-agents are read before its old finished ones.
    let metas = cache.list(dir, |n| n.ends_with(".meta.json"), MAX_AGENT_FILES, now)?;
    // Each row with its file, so rows that tie on time keep one order from poll to poll.
    let mut list: Vec<(Agent, PathBuf)> = Vec::new();
    let (mut running, mut done) = (0usize, 0usize);
    for path in metas {
        cache.used.insert(path.clone());
        let stamp = std::fs::symlink_metadata(&path).ok().and_then(|m| Some((m.modified().ok()?, m.len())));
        if let (Some((t, n)), Some((at, len, a))) = (stamp, cache.done.get(&path)) {
            if t == *at && n == *len {
                done += 1;
                list.push((a.clone(), path));
                continue;
            }
        }
        let Some(stem) = path.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_suffix(".meta.json")) else { continue };
        let Some(v) = read_json(&path, 16 << 10) else { continue };
        if !v.is_object() {
            continue;
        }
        // The transcript's file times: created when the sub-agent started, changed last when it wrote last.
        let times = std::fs::metadata(dir.join(format!("{stem}.jsonl"))).or_else(|_| std::fs::metadata(&path)).ok();
        let start = times.as_ref().and_then(|m| m.created().ok()).map(ms);
        let last = times.as_ref().and_then(|m| m.modified().ok()).map(ms);
        let finished = v["finished"] == Value::Bool(true);
        if !finished && last.map_or(true, |t| now.saturating_sub(t) > STALE_MS) {
            continue;
        }
        if finished { done += 1 } else { running += 1 }
        let text = |k: &str| cut(v[k].as_str().unwrap_or(""), TEXT_CHARS);
        let row = Agent {
            kind: cut(v["agentType"].as_str().unwrap_or(""), 40),
            task: text("description"),
            model: cut(v["model"].as_str().unwrap_or(""), 40),
            since: start,
            done: finished,
            took_s: if finished { start.zip(last).map(|(a, b)| b.saturating_sub(a) / 1000) } else { None },
            depth: v["spawnDepth"].as_u64().unwrap_or(1).saturating_sub(1).min(8) as u32,
        };
        if let (true, Some((t, n))) = (finished, stamp) {
            cache.done.insert(path.clone(), (t, n, row.clone()));
        }
        list.push((row, path));
    }
    if list.is_empty() {
        return None;
    }
    // Running first, the oldest on top; then the finished ones, the latest first; a tie by file name (issue #158: the
    // test fixture's files are all copied in one millisecond).
    list.sort_by(|(a, pa), (b, pb)| {
        a.done.cmp(&b.done).then_with(|| if a.done { b.since.cmp(&a.since) } else { a.since.cmp(&b.since) }).then_with(|| pa.cmp(pb))
    });
    let mut list: Vec<Agent> = list.into_iter().map(|(a, _)| a).collect();
    list.truncate(MAX_AGENT_ROWS);
    Some(Agents { running, done, list })
}

/// Read every session file under `claude` (the `~/.claude` folder). A missing folder gives none; a missing, broken or
/// odd file is skipped; a session whose process is gone (`alive` says no) is skipped. The newest files are read first,
/// and the cap counts live sessions only, so old leftovers cannot push a live session off the board.
#[cfg(test)]
pub fn read(claude: &Path, now: u64, alive: &dyn Fn(i64, &str) -> bool) -> Vec<Local> {
    read_with(claude, now, alive, &mut Cache::default())
}

/// `read`, with what the last poll kept (`Cache`). Each session file is read on every poll: it holds the state.
pub fn read_with(claude: &Path, now: u64, alive: &dyn Fn(i64, &str) -> bool, cache: &mut Cache) -> Vec<Local> {
    let out = read_sessions(claude, now, alive, cache);
    cache.prune();
    out
}

fn read_sessions(claude: &Path, now: u64, alive: &dyn Fn(i64, &str) -> bool, cache: &mut Cache) -> Vec<Local> {
    let Some(files) = cache.list(&claude.join("sessions"), |n| n.ends_with(".json"), MAX_LISTED, now) else { return Vec::new() };
    let mut out = Vec::new();
    for path in files {
        if out.len() >= MAX_SESSIONS {
            break;
        }
        let Some(v) = read_json(&path, MAX_FILE) else { continue };
        if !v.is_object() || !v["pid"].as_i64().map_or(false, |p| alive(p, v["procStart"].as_str().unwrap_or(""))) {
            continue;
        }
        let cwd = v["cwd"].as_str().unwrap_or("");
        let sid = v["sessionId"].as_str().unwrap_or("");
        let agents = if !cwd.is_empty() && safe_id(sid) {
            agents_in(&claude.join("projects").join(slug(cwd)).join(sid).join("subagents"), now, cache)
        } else {
            None
        };
        let bridge = v["bridgeSessionId"].as_str().unwrap_or("");
        out.push(Local {
            tmux: safe_name(v["tmux"].as_str().unwrap_or("").split(':').next().unwrap_or("")),
            rc_id: session_id(&format!("https://claude.ai/code/{bridge}")),
            entrypoint: cut(v["entrypoint"].as_str().unwrap_or(""), 40),
            interactive: v["kind"] == "interactive",
            status: cut(v["status"].as_str().unwrap_or(""), 20),
            waiting_for: cut(v["waitingFor"].as_str().unwrap_or(""), 40),
            title: cut(v["name"].as_str().unwrap_or(""), TEXT_CHARS),
            project: project_of(cwd),
            since: v["statusUpdatedAt"].as_i64().or_else(|| v["startedAt"].as_i64()).unwrap_or(0),
            agents,
        });
    }
    out
}

/// A fresh copy of the fixture folder, so the sub-agent files have new times (a running one is not stale).
#[cfg(test)]
pub fn fixture() -> std::path::PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/local/claude");
    // A counter, not only the time: tests run in parallel and two can start in the same millisecond (issue #158).
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dst = std::env::temp_dir().join(format!("pod-board-local-{}-{}-{n}", std::process::id(), ms(SystemTime::now())));
    fn copy(a: &Path, b: &Path) {
        std::fs::create_dir_all(b).unwrap();
        // In name order: the copies' times then follow the names (agent 1 starts first), whatever order the
        // folder lists them in and however many milliseconds the copy takes (issue #158).
        let mut entries: Vec<_> = std::fs::read_dir(a).unwrap().flatten().map(|e| e.path()).collect();
        entries.sort();
        for path in entries {
            let to = b.join(path.file_name().unwrap());
            if path.is_dir() { copy(&path, &to) } else { std::fs::copy(&path, &to).unwrap(); }
        }
    }
    copy(&src, &dst);
    dst
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> u64 { ms(SystemTime::now()) }
    fn all_alive(_: i64, _: &str) -> bool { true }

    #[test]
    fn every_readable_session_file_is_read_and_broken_ones_are_skipped() {
        let dir = fixture();
        let got = read(&dir, now(), &all_alive);
        let mut names: Vec<&str> = got.iter().map(|l| l.title.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["Judge run", "No remote control", "Pod Bay twin", "Print run", "Quiet terminal", "Remote twin", "Sub-agent host", "Waits on a push"]);
        let host = got.iter().find(|l| l.title == "Sub-agent host").unwrap();
        assert_eq!((host.project.as_str(), host.status.as_str(), host.rc_id.as_str()), ("alpha", "busy", "session_01FixtureHost"));
        let twin = got.iter().find(|l| l.title == "Pod Bay twin").unwrap();
        assert_eq!(twin.tmux, "claude-pb-bot-02");
    }

    #[test]
    fn a_missing_folder_gives_nothing_and_a_dead_process_is_skipped() {
        assert!(read(Path::new("/nonexistent/pod-board"), now(), &all_alive).is_empty());
        let dir = fixture();
        assert!(read(&dir, now(), &|_, _| false).is_empty());
        let me = std::process::id() as i64;
        assert!(alive(me, ""));
        assert!(!alive(0, "") && !alive(-5, ""));
        #[cfg(target_os = "linux")]
        {
            let start = linux_start(me).expect("own start time");
            assert!(alive(me, &start), "the same process");
            assert!(!alive(me, "1"), "a reused pid: another start time is another process");
        }
    }

    #[test]
    fn five_running_and_two_done_sub_agents_with_depth() {
        let dir = fixture();
        let got = read(&dir, now(), &all_alive);
        let a = got.iter().find(|l| l.title == "Sub-agent host").unwrap().agents.clone().expect("sub-agents");
        assert_eq!((a.running, a.done, a.list.len()), (5, 2, 7));
        assert!(a.list[..5].iter().all(|x| !x.done), "running rows come first");
        let nested = a.list.iter().find(|x| x.task == "Nested lookup").unwrap();
        assert_eq!((nested.depth, nested.kind.as_str(), nested.model.as_str()), (1, "Explore", "haiku"));
        assert_eq!(a.list.iter().find(|x| x.task == "Scan the model").unwrap().depth, 0);
        assert!(a.list.iter().filter(|x| x.done).all(|x| x.took_s.is_some()));
        assert!(got.iter().find(|l| l.title == "Quiet terminal").unwrap().agents.is_none(), "a session with none reports none");
    }

    #[test]
    fn many_dead_files_cannot_push_a_live_session_off_the_board() {
        let dir = std::env::temp_dir().join(format!("pod-board-many-{}-{}", std::process::id(), now()));
        std::fs::create_dir_all(dir.join("sessions")).unwrap();
        let live = r#"{"pid": 4242, "kind": "interactive", "entrypoint": "cli", "status": "idle", "name": "Live one", "cwd": "/w/alpha"}"#;
        std::fs::write(dir.join("sessions/9999.json"), live).unwrap();
        // Written after the live one, so they are newer and come first.
        for i in 0..(MAX_SESSIONS + 10) {
            std::fs::write(dir.join(format!("sessions/{:04}.json", i)), format!(r#"{{"pid": {}, "status": "idle"}}"#, 10_000 + i)).unwrap();
        }
        let got = read(&dir, now(), &|pid, _| pid == 4242);
        assert_eq!(got.iter().map(|l| l.title.as_str()).collect::<Vec<_>>(), vec!["Live one"]);
    }

    #[test]
    fn a_stale_unfinished_sub_agent_is_not_running() {
        let dir = fixture();
        let later = now() + STALE_MS + 60_000;
        let a = read(&dir, later, &all_alive).into_iter().find(|l| l.title == "Sub-agent host").unwrap().agents.unwrap();
        assert_eq!((a.running, a.done), (0, 2));
    }

    #[test]
    fn slugs_ids_and_projects() {
        assert_eq!(slug("/home/u/my.repo/src-03/pb_bot"), "-home-u-my-repo-src-03-pb-bot");
        assert!(safe_id("0f1e2d3c-aaaa-4bbb-8ccc-1234567890ab"));
        for bad in ["", "../x", "a/b", "zz", "0f1e/.."] { assert!(!safe_id(bad), "{bad}"); }
        assert_eq!(project_of("/home/someone/secret/alpha"), "alpha");
        assert_eq!(project_of("/home/someone/beta/"), "beta");
        for home in ["/home/someone", "/Users/someone/", "/root", "/", ""] { assert_eq!(project_of(home), "~", "{home}"); }
        assert_eq!(cut("a\nb\u{202E}c\u{2066}d\u{200B}e\u{FEFF}f\u{061C}g", 80), "abcdefg");
        assert_eq!(home_free("Read /home/someone/secret/a.rs and /Users/x/b, /root/c, /rooted, /root"), "Read ~/secret/a.rs and ~/b, ~/c, /rooted, ~");
        assert_eq!(home_free("plain text"), "plain text");
    }

    #[test]
    fn two_fixtures_made_at_once_are_two_folders() {
        // Issue #158 bug 1: tests that start in the same millisecond shared one copy and rewrote each other's files.
        let made: Vec<_> = (0..8).map(|_| fixture()).collect();
        let mut unique = made.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), made.len(), "{made:?}");
    }

    /// The login name as the tests see it: the last folder of the real `$HOME`.
    fn login() -> (String, String) {
        let home = std::env::var("HOME").expect("HOME is set").trim_end_matches('/').to_string();
        let name = home.rsplit('/').next().unwrap().to_string();
        assert!(!name.is_empty(), "HOME={home}");
        (home, name)
    }

    #[test]
    fn no_login_name_or_absolute_path_reaches_the_project_or_a_task() {
        // Issue #158 bug 3: other home layouts, the real $HOME, and a path outside any home.
        let (home, user) = login();
        for cwd in [format!("/var/home/{user}"), format!("/export/home/{user}/"), home.clone(), format!("/srv/data/{user}")] {
            assert_eq!(project_of(&cwd), "~", "{cwd}");
        }
        assert_eq!(project_of(&format!("/var/home/{user}/alpha")), "alpha");
        assert_eq!(project_of(&format!("/export/home/{user}/work/beta")), "beta");
        let task = format!("Read {home}/a.rs, /var/home/{user}/b.rs, (/export/home/{user}/c) and /srv/data/{user}/secret/d.txt; see https://x.test/a/b");
        let got = cut(&task, 400);
        assert_eq!(got, "Read ~/a.rs, ~/b.rs, (~/c) and …/d.txt; see https://x.test/a/b");
        assert!(!got.contains(&user), "{got}");
    }

    #[test]
    fn the_model_of_a_session_in_another_home_layout_holds_no_login_name_or_path() {
        let (home, user) = login();
        let dir = std::env::temp_dir().join(format!("pod-board-privacy-{}-{}", std::process::id(), now()));
        let cwd = format!("/var/home/{user}");
        let sid = "0f1e2d3c-aaaa-4bbb-8ccc-1234567890ab";
        std::fs::create_dir_all(dir.join("sessions")).unwrap();
        let session = serde_json::json!({"pid": 4242, "kind": "interactive", "entrypoint": "cli", "status": "waiting",
            "waitingFor": format!("input needed in {home}/x"), "name": "", "cwd": cwd, "sessionId": sid,
            "bridgeSessionId": "session_01Privacy"});
        std::fs::write(dir.join("sessions/4242.json"), session.to_string()).unwrap();
        let agents = dir.join("projects").join(slug(&cwd)).join(sid).join("subagents");
        std::fs::create_dir_all(&agents).unwrap();
        let meta = serde_json::json!({"agentType": "Explore", "description": format!("Scan /export/home/{user}/repo and /opt/{user}/build/out.log"),
            "model": "haiku", "spawnDepth": 1, "finished": false});
        std::fs::write(agents.join("agent-a1.meta.json"), meta.to_string()).unwrap();
        std::fs::write(agents.join("agent-a1.jsonl"), "{}").unwrap();
        let local = read(&dir, now(), &all_alive);
        assert_eq!(local.len(), 1);
        let model = crate::model::build(&crate::model::Raw {
            ls: serde_json::json!([{"machine": "UA", "result": []}]),
            machines: serde_json::json!([{"machine": "UA", "me": true}]),
            local,
            ..Default::default()
        });
        assert_eq!(model.sessions.len(), 1, "the terminal session reaches the model");
        let text = serde_json::to_string(&model).unwrap();
        assert!(text.contains("repo") && text.contains("out.log"), "{text}");
        assert!(!text.contains(&user) && !text.contains(&home) && !text.contains("/opt") && !text.contains("/export"), "{text}");
    }

    /// Set a folder's change time to `secs` (seconds since the epoch).
    #[cfg(unix)]
    fn set_time(path: &Path, secs: i64) {
        use std::os::unix::ffi::OsStrExt;
        let c = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
        let t = libc::timespec { tv_sec: secs as libc::time_t, tv_nsec: 0 };
        assert_eq!(unsafe { libc::utimensat(libc::AT_FDCWD, c.as_ptr(), [t, t].as_ptr(), 0) }, 0);
    }

    #[cfg(unix)]
    #[test]
    fn a_poll_reuses_the_listing_of_an_unchanged_folder_and_the_rows_of_finished_sub_agents() {
        // Issue #158 bug 8: a poll listed and stat'ed every file of `sessions/` and of each `subagents/` folder.
        let dir = fixture();
        let old = (now() / 1000) as i64 - 30;
        let subs = dir.join("projects/-home-someone-secret-alpha/11111111-1111-4111-8111-111111111111/subagents");
        let (sessions, mut cache) = (dir.join("sessions"), Cache::default());
        set_time(&sessions, old);
        set_time(&subs, old);
        let host = |got: &[Local]| got.iter().find(|l| l.title == "Sub-agent host").unwrap().agents.clone().unwrap();
        let finished = std::fs::read_dir(&subs).unwrap().flatten().map(|e| e.path())
            .find(|p| std::fs::read_to_string(p).map_or(false, |t| t.contains("\"Finished one\""))).unwrap();
        set_time(&finished, old);
        set_time(&subs, old);
        let first = read_with(&dir, now(), &all_alive, &mut cache);
        assert_eq!((host(&first).running, host(&first).done), (5, 2));

        // A file added while the folder time stays the same, and a finished sub-agent's file rewritten with the same
        // size and change time: neither is seen, so neither the folder nor that file was read again.
        std::fs::write(sessions.join("7777.json"), r#"{"pid": 7777, "kind": "interactive", "entrypoint": "cli", "status": "idle", "name": "Newcomer", "cwd": "/w/new"}"#).unwrap();
        let text = std::fs::read_to_string(&finished).unwrap();
        std::fs::write(&finished, text.replace("Finished one", "Finished two")).unwrap();
        set_time(&finished, old);
        set_time(&sessions, old);
        set_time(&subs, old);
        let second = read_with(&dir, now(), &all_alive, &mut cache);
        assert!(second.iter().all(|l| l.title != "Newcomer"), "the listing of the unchanged folder is used again");
        assert!(host(&second).list.iter().any(|a| a.task == "Finished one"), "a finished sub-agent is not read again");
        assert_eq!(second.iter().filter(|l| l.title != "Newcomer").count(), first.len(), "each session file is still read");

        // The folder changed: it is listed again. The finished file changed (review of #158, F6): it is read again.
        set_time(&sessions, old + 1);
        std::fs::write(&finished, r#"{"agentType": "general-purpose", "description": "Rewritten", "model": "sonnet", "spawnDepth": 1, "finished": true}"#).unwrap();
        set_time(&subs, old);
        let third = read_with(&dir, now(), &all_alive, &mut cache);
        assert!(third.iter().any(|l| l.title == "Newcomer"));
        assert!(host(&third).list.iter().any(|a| a.task == "Rewritten"), "a changed finished file is read again");
    }

    #[test]
    fn a_folder_changed_just_now_is_listed_again() {
        // A file added in the same clock tick as the last listing leaves the folder time as it was: no reuse then.
        let dir = fixture();
        let mut cache = Cache::default();
        let first = read_with(&dir, now(), &all_alive, &mut cache).len();
        std::fs::write(dir.join("sessions/7778.json"), r#"{"pid": 7778, "kind": "interactive", "entrypoint": "cli", "status": "idle", "name": "Quick", "cwd": "/w/q"}"#).unwrap();
        let got = read_with(&dir, now(), &all_alive, &mut cache);
        assert_eq!(got.len(), first + 1);
    }

    #[test]
    fn a_path_behind_a_hidden_character_a_colon_or_a_comma_loses_the_login() {
        // Security review of #158: M1 (hidden character), M2 (`file://`, `host:`, comma), M3 (a path ending at the login).
        let (home, user) = login();
        let cases = [
            format!("see \u{200B}/home/{user}/x"),
            format!("open file:///home/{user}/a"),
            format!("copy host:/home/{user}/x"),
            format!("a.rs,/home/{user}/b and -v{home}/w:/w"),
            format!("in /srv/data/{user} and /{user}"),
            format!("cd ~/.claude/projects/{user}/repo"),
            format!("ask /Users/{user}x/y and other:/var/home/bob/z"),
        ];
        for c in &cases {
            let got = cut(c, 400);
            assert!(!got.contains(&user), "{c} -> {got}");
            assert!(!got.contains("/home/bob") && !got.contains(&home), "{c} -> {got}");
        }
        assert_eq!(cut(&format!("see \u{200B}/home/{user}/x"), 80), "see ~/x");
        assert_eq!(cut(&format!("in /srv/data/{user} and /{user}"), 80), "in ~ and ~");
        assert_eq!(cut("see https://x.test/a/b, ok", 80), "see https://x.test/a/b, ok");
        // Only whole folder names change (review of #158, F4).
        assert_eq!(cut(&format!("edit src/{user}ice.rs and host:/home/{user}ice/x"), 200), format!("edit src/{user}ice.rs and host:/home/~/x"));
        assert_eq!(cut(&format!("copy host:/home/{user}/x,y"), 80), "copy host:/home/~/x,y");
    }
}

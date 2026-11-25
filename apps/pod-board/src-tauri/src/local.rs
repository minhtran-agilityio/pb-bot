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
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// At most this many session files are read in one poll, and each one at most this big.
const MAX_SESSIONS: usize = 64;
const MAX_FILE: u64 = 64 << 10;
/// At most this many sub-agent files of one session are read; at most this many rows reach the page.
const MAX_AGENT_FILES: usize = 400;
const MAX_AGENT_ROWS: usize = 40;
/// A sub-agent with no `finished` mark whose transcript has not changed for this long is not counted as running: it
/// was stopped without its mark (a crash, a killed session). A running one writes to its transcript all the time.
pub const STALE_MS: u64 = 15 * 60 * 1000;
const TEXT_CHARS: usize = 80;

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

/// Shorten a text from a file, and drop control and text-direction characters (they could disguise a notification).
fn cut(s: &str, n: usize) -> String {
    let clean: String = s.chars().filter(|c| !c.is_control() && !matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}')).collect();
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
    if !std::fs::symlink_metadata(path).ok()?.file_type().is_file() {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
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

/// The last folder name of a path, as the project. A home folder (`/home/<user>`, `/Users/<user>`, `/root`) is `~`,
/// so the login name never shows.
fn project_of(cwd: &str) -> String {
    let path = cwd.trim_end_matches('/');
    let (parent, last) = path.rsplit_once('/').unwrap_or(("", path));
    if last.is_empty() || path == "/root" || parent == "/home" || parent == "/Users" {
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

/// Whether process `pid` still runs (a session file outlives a session only when it was killed hard).
pub fn alive(pid: i64) -> bool {
    #[cfg(unix)]
    {
        if pid <= 0 || pid > i32::MAX as i64 {
            return false;
        }
        // Signal 0 checks the process without touching it. EPERM: it runs under another user.
        let r = unsafe { libc::kill(pid as libc::pid_t, 0) };
        r == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

/// The sub-agents of one session, from its `subagents` folder. None when it has none.
pub fn agents_in(dir: &Path, now: u64) -> Option<Agents> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut list: Vec<Agent> = Vec::new();
    let (mut running, mut done) = (0usize, 0usize);
    for e in entries.flatten().take(MAX_AGENT_FILES * 2) {
        let path = e.path();
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
        list.push(Agent {
            kind: cut(v["agentType"].as_str().unwrap_or(""), 40),
            task: text("description"),
            model: cut(v["model"].as_str().unwrap_or(""), 40),
            since: start,
            done: finished,
            took_s: if finished { start.zip(last).map(|(a, b)| b.saturating_sub(a) / 1000) } else { None },
            depth: v["spawnDepth"].as_u64().unwrap_or(1).saturating_sub(1).min(8) as u32,
        });
    }
    if list.is_empty() {
        return None;
    }
    // Running first, the oldest on top; then the finished ones, the latest first.
    list.sort_by(|a, b| a.done.cmp(&b.done).then_with(|| if a.done { b.since.cmp(&a.since) } else { a.since.cmp(&b.since) }));
    list.truncate(MAX_AGENT_ROWS);
    Some(Agents { running, done, list })
}

/// Read every session file under `claude` (the `~/.claude` folder). A missing folder gives none; a missing, broken or
/// odd file is skipped; a session whose process is gone (`alive` says no) is skipped.
pub fn read(claude: &Path, now: u64, alive: &dyn Fn(i64) -> bool) -> Vec<Local> {
    let Ok(entries) = std::fs::read_dir(claude.join("sessions")) else { return Vec::new() };
    let mut files: Vec<_> = entries.flatten().map(|e| e.path()).filter(|p| p.extension().map_or(false, |x| x == "json")).collect();
    files.sort();
    let mut out = Vec::new();
    for path in files.into_iter().take(MAX_SESSIONS) {
        let Some(v) = read_json(&path, MAX_FILE) else { continue };
        if !v.is_object() || !v["pid"].as_i64().map_or(false, |p| alive(p)) {
            continue;
        }
        let cwd = v["cwd"].as_str().unwrap_or("");
        let sid = v["sessionId"].as_str().unwrap_or("");
        let agents = if !cwd.is_empty() && safe_id(sid) {
            agents_in(&claude.join("projects").join(slug(cwd)).join(sid).join("subagents"), now)
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
    let dst = std::env::temp_dir().join(format!("pod-board-local-{}-{}", std::process::id(), ms(SystemTime::now())));
    fn copy(a: &Path, b: &Path) {
        std::fs::create_dir_all(b).unwrap();
        for e in std::fs::read_dir(a).unwrap().flatten() {
            let to = b.join(e.file_name());
            if e.path().is_dir() { copy(&e.path(), &to) } else { std::fs::copy(e.path(), &to).unwrap(); }
        }
    }
    copy(&src, &dst);
    dst
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> u64 { ms(SystemTime::now()) }
    fn all_alive(_: i64) -> bool { true }

    #[test]
    fn every_readable_session_file_is_read_and_broken_ones_are_skipped() {
        let dir = fixture();
        let got = read(&dir, now(), &all_alive);
        let mut names: Vec<&str> = got.iter().map(|l| l.title.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["No remote control", "Pod Bay twin", "Print run", "Quiet terminal", "Remote twin", "Sub-agent host", "Waits on a push"]);
        let host = got.iter().find(|l| l.title == "Sub-agent host").unwrap();
        assert_eq!((host.project.as_str(), host.status.as_str(), host.rc_id.as_str()), ("alpha", "busy", "session_01FixtureHost"));
        let twin = got.iter().find(|l| l.title == "Pod Bay twin").unwrap();
        assert_eq!(twin.tmux, "claude-pb-bot-02");
    }

    #[test]
    fn a_missing_folder_gives_nothing_and_a_dead_process_is_skipped() {
        assert!(read(Path::new("/nonexistent/pod-board"), now(), &all_alive).is_empty());
        let dir = fixture();
        assert!(read(&dir, now(), &|_| false).is_empty());
        assert!(alive(std::process::id() as i64));
        assert!(!alive(0) && !alive(-5));
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
        assert_eq!(cut("a\nb\u{202E}c\u{2066}d", 80), "abcd");
    }
}

//! The three checks the Add project dialog runs before it saves a project:
//! `gh auth status`, `claude --version`, and the repo folder (`git -C <path> remote get-url origin`
//! must point at the GitHub repo the owner typed).

use serde::Serialize;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Shown wherever the repo input is not `owner/name` (or a form that holds one).
pub const REPO_FORMAT_ERROR: &str = "Use the form owner/name, for example acme/shop-web.";
/// Shown when the folder is not a full path.
pub const FULL_PATH_ERROR: &str = "Use a full path, for example ~/code/shop-web.";

const PATH_BEGIN: &str = "__SHAPEBOARD_PATH_BEGIN__";
const PATH_END: &str = "__SHAPEBOARD_PATH_END__";
/// How long app start waits for the owner's shell to print its PATH.
const SHELL_PATH_TIMEOUT: Duration = Duration::from_secs(3);
/// How long `gh`, `claude` and `git` may take. `gh auth status` goes to the network.
const CHECK_TIMEOUT: Duration = Duration::from_secs(15);
/// How long `ssh -G <alias>` may take. It reads config only and does not touch the network.
const SSH_G_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub ok: bool,
    /// One line for the UI: the version, or what went wrong and how to fix it.
    pub detail: String,
    pub ms: u128,
}

extern "C" {
    fn setsid() -> i32;
    fn kill(pid: i32, sig: i32) -> i32;
}

/// What a bounded command printed. `status` is `None` when the output ended the wait early.
struct Captured {
    status: Option<ExitStatus>,
    stdout: String,
    stderr: String,
}

enum CaptureError {
    Spawn(std::io::Error),
    Timeout,
}

/// Copies a pipe into a shared buffer on its own thread; `finished` turns true at EOF.
fn pump(mut pipe: impl Read + Send + 'static) -> (Arc<Mutex<Vec<u8>>>, Arc<AtomicBool>) {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let finished = Arc::new(AtomicBool::new(false));
    let (b, f) = (buf.clone(), finished.clone());
    std::thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        while let Ok(n) = pipe.read(&mut chunk) {
            if n == 0 {
                break;
            }
            b.lock().unwrap_or_else(|e| e.into_inner()).extend_from_slice(&chunk[..n]);
        }
        f.store(true, Ordering::SeqCst);
    });
    (buf, finished)
}

fn text(buf: &Mutex<Vec<u8>>) -> String {
    String::from_utf8_lossy(&buf.lock().unwrap_or_else(|e| e.into_inner())).into_owned()
}

/// Runs `cmd` with no stdin, in its own session (so it has no terminal to prompt on or grab,
/// and `kill(-pid)` reaches everything it started). Returns when the command exits, when
/// `done(stdout so far)` is true, or at `timeout`; then the whole process group is killed, so
/// a background process the command left behind cannot keep the pipes open or run on.
/// `Err(Timeout)` means neither happened in time.
fn capture(mut cmd: Command, timeout: Duration, done: Option<fn(&str) -> bool>) -> Result<Captured, CaptureError> {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    // SAFETY: `setsid` is async-signal-safe, the only thing allowed between fork and exec.
    // Its result is ignored: it fails only for a process group leader, which a new child is not.
    unsafe {
        cmd.pre_exec(|| {
            setsid();
            Ok(())
        });
    }
    let mut child = cmd.spawn().map_err(CaptureError::Spawn)?;
    let (out, out_done) = pump(child.stdout.take().ok_or(CaptureError::Timeout)?);
    let (err, err_done) = pump(child.stderr.take().ok_or(CaptureError::Timeout)?);
    // After `setsid` the child's pid is also its process group id.
    let kill_group = |child: &mut std::process::Child| {
        // SAFETY: plain syscall; a negative pid addresses the group. The child is not reaped
        // yet, so its pid cannot belong to anything else.
        unsafe { kill(-(child.id() as i32), 9) };
        let _ = child.kill();
    };
    let deadline = Instant::now() + timeout;
    let (status, timed_out) = loop {
        if let Ok(Some(st)) = child.try_wait() {
            break (Some(st), false);
        }
        if done.is_some_and(|d| d(&text(&out))) {
            break (None, false);
        }
        if Instant::now() >= deadline {
            break (None, true);
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    kill_group(&mut child);
    let _ = child.wait();
    if timed_out {
        return Err(CaptureError::Timeout);
    }
    // The group is dead, so both pipes close at once; give the pumps a moment to drain.
    let drain = Instant::now() + Duration::from_millis(500);
    while !(out_done.load(Ordering::SeqCst) && err_done.load(Ordering::SeqCst)) && Instant::now() < drain {
        std::thread::sleep(Duration::from_millis(2));
    }
    Ok(Captured { status, stdout: text(&out), stderr: text(&err) })
}

/// Returns the stdout of `cmd`, or `None` when it cannot start or does not finish within
/// `timeout`. The exit status is ignored: the caller parses the output (an interactive shell
/// may exit non-zero after it printed). With `done`, stops as soon as `done` accepts the stdout.
fn stdout_with_timeout(cmd: Command, timeout: Duration, done: Option<fn(&str) -> bool>) -> Option<String> {
    capture(cmd, timeout, done).ok().map(|c| c.stdout)
}

/// The text between the last begin marker and the next end marker. An interactive shell may
/// print noise before and after (motd, plugin banners, escape codes).
fn between_markers(out: &str) -> Option<&str> {
    let start = out.rfind(PATH_BEGIN)? + PATH_BEGIN.len();
    let end = out[start..].find(PATH_END)? + start;
    Some(&out[start..end])
}

/// The PATH of the owner's interactive login shell, so nvm, volta and asdf (set in `.zshrc`)
/// count. `None` when the shell fails or does not answer within the timeout.
fn shell_path(shell: &str, timeout: Duration) -> Option<String> {
    let script = format!("printf '%s%s%s' '{PATH_BEGIN}' \"$PATH\" '{PATH_END}'");
    let mut cmd = Command::new(shell);
    cmd.args(["-ilc", &script]);
    let out = stdout_with_timeout(cmd, timeout, Some(|o| between_markers(o).is_some()))?;
    between_markers(&out).map(str::to_string).filter(|p| !p.is_empty())
}

/// The PATH the checks give to the programs they start (`gh`, `claude`, `git`, `ssh`).
static GUI_PATH: OnceLock<String> = OnceLock::new();
/// Becomes true when the PATH lookup finished.
static PATH_READY: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());
static PATH_STARTED: AtomicBool = AtomicBool::new(false);

/// The PATH for an app started from Finder, which only gets a short one (/usr/bin:/bin:...):
/// the PATH of the owner's interactive login shell (at most `SHELL_PATH_TIMEOUT`), plus the
/// usual install folders, plus the current PATH.
fn compute_gui_path() -> String {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let mut parts: Vec<String> = Vec::new();
    if let Some(path) = shell_path(&shell, SHELL_PATH_TIMEOUT) {
        parts.extend(path.split(':').map(String::from));
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        for sub in [".local/bin", ".claude/local", ".npm-global/bin"] {
            parts.push(home.join(sub).to_string_lossy().into_owned());
        }
    }
    parts.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(String::from));
    if let Ok(cur) = std::env::var("PATH") {
        parts.extend(cur.split(':').map(String::from));
    }
    let mut seen = std::collections::HashSet::new();
    parts.retain(|p| !p.is_empty() && seen.insert(p.clone()));
    parts.join(":")
}

/// Starts the PATH lookup on a background thread, so the window opens at once. Every program
/// the checks start waits for it (`command`), for at most the lookup's own timeout.
/// The result is passed to each child as `PATH`; the app's own environment is not changed.
pub fn start_path_fix() {
    PATH_STARTED.store(true, Ordering::SeqCst);
    std::thread::spawn(|| {
        let _ = GUI_PATH.set(compute_gui_path());
        mark_ready(&PATH_READY);
    });
}

fn mark_ready(state: &(Mutex<bool>, Condvar)) {
    *state.0.lock().unwrap_or_else(|e| e.into_inner()) = true;
    state.1.notify_all();
}

/// Waits until `state` is ready, at most `timeout`. Returns whether it is ready.
fn wait_ready(state: &(Mutex<bool>, Condvar), timeout: Duration) -> bool {
    let guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
    match state.1.wait_timeout_while(guard, timeout, |ready| !*ready) {
        Ok((ready, _)) => *ready,
        Err(e) => *e.into_inner().0,
    }
}

/// A `Command` with the shell's PATH. Waits for the lookup if it is still running; when it
/// does not finish in time, the child gets the app's own PATH.
fn command(program: &str) -> Command {
    if PATH_STARTED.load(Ordering::SeqCst) {
        wait_ready(&PATH_READY, SHELL_PATH_TIMEOUT + Duration::from_secs(1));
    }
    let mut cmd = Command::new(program);
    if let Some(path) = GUI_PATH.get() {
        cmd.env("PATH", path);
    }
    cmd
}

/// `~/code/x` -> `/Users/me/code/x`. Other paths are returned unchanged.
pub fn expand_home(path: &str) -> PathBuf {
    let path = path.trim();
    if path == "~" || path.starts_with("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(path.trim_start_matches('~').trim_start_matches('/'));
        }
    }
    PathBuf::from(path)
}

/// Expands `~`, then requires a full path. A relative path would resolve against the app's
/// working folder, which is never what the owner meant.
pub fn resolve_folder(path: &str) -> Result<PathBuf, String> {
    let p = expand_home(path);
    if p.is_absolute() {
        Ok(p)
    } else {
        Err(FULL_PATH_ERROR.to_string())
    }
}

/// `owner/name` with an optional `.git` suffix; each part is letters, digits, `-`, `_` or `.`.
fn owner_name(s: &str) -> Option<String> {
    let s = s.trim().trim_end_matches('/');
    let s = s.strip_suffix(".git").unwrap_or(s);
    let mut it = s.split('/');
    let (owner, name) = (it.next()?, it.next()?);
    let valid = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    if it.next().is_some() || !valid(owner) || !valid(name) {
        return None;
    }
    Some(format!("{owner}/{name}"))
}

/// Accepts `owner/name`, with an optional `https://github.com/` (or `www.`) prefix or `.git` suffix.
pub fn normalize_repo(input: &str) -> Option<String> {
    let s = input.trim();
    let s = ["https://www.github.com/", "https://github.com/", "http://github.com/", "github.com/"]
        .iter()
        .find_map(|p| s.strip_prefix(p))
        .unwrap_or(s);
    owner_name(s)
}

/// A git remote URL split into its host and repo path.
#[derive(Debug, PartialEq)]
struct Remote {
    host: String,
    path: String,
    /// True for `git@host:o/n` and `ssh://...`: the host may be an alias from `~/.ssh/config`.
    ssh: bool,
}

fn parse_remote(url: &str) -> Option<Remote> {
    let url = url.trim();
    let (host, path, ssh) = if let Some((scheme, rest)) = url.split_once("://") {
        let ssh = match scheme.to_ascii_lowercase().as_str() {
            "ssh" | "git+ssh" | "ssh+git" => true,
            "https" | "http" | "git" => false,
            _ => return None,
        };
        let (auth, path) = rest.split_once('/')?;
        // Drop `user:password@` and `:port`.
        let host_port = auth.rsplit_once('@').map_or(auth, |(_, h)| h);
        let host = match host_port.rsplit_once(':') {
            Some((h, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => h,
            Some(_) => return None,
            None => host_port,
        };
        (host, path, ssh)
    } else {
        // scp-like: `[user@]host:path`
        let (auth, path) = url.split_once(':')?;
        (auth.rsplit_once('@').map_or(auth, |(_, h)| h), path, true)
    };
    if host.is_empty() || host.contains('/') {
        return None;
    }
    Some(Remote { host: host.to_ascii_lowercase(), path: path.trim_start_matches('/').to_string(), ssh })
}

fn is_github_host(h: &str) -> bool {
    h == "github.com" || h == "www.github.com"
}

/// The `hostname` value in the output of `ssh -G <alias>`.
fn hostname_from_ssh_g(out: &str) -> Option<String> {
    out.lines()
        .find_map(|l| l.strip_prefix("hostname "))
        .map(|h| h.trim().to_ascii_lowercase())
        .filter(|h| !h.is_empty())
}

/// Resolves an SSH host alias (`Host github-work` in `~/.ssh/config`) to its real host name.
fn ssh_resolve_host(alias: &str) -> Option<String> {
    let safe = alias.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c));
    if alias.is_empty() || alias.starts_with('-') || !safe {
        return None;
    }
    let mut cmd = command("ssh");
    cmd.args(["-G", alias]);
    hostname_from_ssh_g(&stdout_with_timeout(cmd, SSH_G_TIMEOUT, None)?)
}

/// Like `repo_from_remote`, with the SSH alias lookup passed in so tests need no `ssh`.
fn repo_from_remote_with(url: &str, resolve: impl Fn(&str) -> Option<String>) -> Option<String> {
    let r = parse_remote(url)?;
    let github = is_github_host(&r.host)
        || (r.ssh && resolve(&r.host).is_some_and(|h| is_github_host(&h) || h == "ssh.github.com"));
    if !github {
        return None;
    }
    owner_name(&r.path)
}

/// `git@github.com:o/n.git`, `https://github.com/o/n`, `https://user:token@github.com/o/n`,
/// `ssh://git@github.com:22/o/n.git`, `git@github-work:o/n.git` (alias) -> `o/n`.
pub fn repo_from_remote(url: &str) -> Option<String> {
    repo_from_remote_with(url, ssh_resolve_host)
}

/// The URL without `user:password@`, safe to show in an error message.
fn redact_url(url: &str) -> String {
    let url = url.trim();
    if let Some((scheme, rest)) = url.split_once("://") {
        let auth_end = rest.find('/').unwrap_or(rest.len());
        if let Some(at) = rest[..auth_end].rfind('@') {
            return format!("{scheme}://{}", &rest[at + 1..]);
        }
    }
    url.to_string()
}

enum RunError {
    NotInstalled,
    Timeout,
    /// The command ran and failed: its whole stderr (or stdout when stderr is empty).
    Failed(String),
}

impl RunError {
    /// One line for the UI.
    fn message(&self, program: &str) -> String {
        match self {
            RunError::NotInstalled => format!("{program} is not installed or not on PATH."),
            RunError::Timeout => format!("{program} did not answer within {} s.", CHECK_TIMEOUT.as_secs()),
            RunError::Failed(m) => m.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("failed").to_string(),
        }
    }
}

/// Runs a program for at most `timeout`. Success gives stdout (stderr when stdout is empty).
fn try_run(program: &str, args: &[&str], timeout: Duration) -> Result<String, RunError> {
    let mut cmd = command(program);
    cmd.args(args);
    match capture(cmd, timeout, None) {
        Err(CaptureError::Timeout) => Err(RunError::Timeout),
        Err(CaptureError::Spawn(e)) if e.kind() == std::io::ErrorKind::NotFound => Err(RunError::NotInstalled),
        Err(CaptureError::Spawn(e)) => Err(RunError::Failed(format!("{program} could not start: {e}"))),
        Ok(c) => {
            let (stdout, stderr) = (c.stdout.trim().to_string(), c.stderr.trim().to_string());
            if c.status.is_some_and(|s| s.success()) {
                Ok(if stdout.is_empty() { stderr } else { stdout })
            } else {
                Err(RunError::Failed(if stderr.is_empty() { stdout } else { stderr }))
            }
        }
    }
}

fn run(program: &str, args: &[&str]) -> Result<String, String> {
    try_run(program, args, CHECK_TIMEOUT).map_err(|e| e.message(program))
}

fn timed(f: impl FnOnce() -> Result<String, String>) -> CheckResult {
    let start = Instant::now();
    let r = f();
    let ms = start.elapsed().as_millis();
    match r {
        Ok(detail) => CheckResult { ok: true, detail, ms },
        Err(detail) => CheckResult { ok: false, detail, ms },
    }
}

/// What `gh auth status` said, as one line for the UI. Only a "not logged in" text gets the
/// sign-in advice; a timeout or a network error says what it was.
fn gh_error_message(e: &RunError) -> String {
    const SIGNED_OUT: [&str; 5] = ["not logged in", "gh auth login", "failed to log in", "is invalid", "no oauth token"];
    match e {
        RunError::Failed(m) if SIGNED_OUT.iter().any(|t| m.to_ascii_lowercase().contains(t)) => {
            "gh is not signed in. Run `gh auth login` in a terminal.".to_string()
        }
        // gh prints the host name on its own first line; skip it.
        RunError::Failed(m) => m
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty() && *l != "github.com")
            .unwrap_or("gh auth status failed.")
            .to_string(),
        other => other.message("gh auth status"),
    }
}

pub fn check_gh() -> CheckResult {
    timed(|| {
        try_run("gh", &["auth", "status", "--hostname", "github.com"], CHECK_TIMEOUT)
            .map(|_| "Signed in to github.com".to_string())
            .map_err(|e| match e {
                RunError::NotInstalled => e.message("gh"),
                _ => gh_error_message(&e),
            })
    })
}

pub fn check_claude() -> CheckResult {
    timed(|| run("claude", &["--version"]).map(|v| v.lines().next().unwrap_or("").to_string()))
}

pub fn check_folder(repo: &str, path: &str) -> CheckResult {
    timed(|| check_folder_inner(repo, &resolve_folder(path)?))
}

/// `/Users/me/code/x` -> `~/code/x`.
fn with_tilde(p: &Path) -> String {
    if let Some(home) = std::env::var_os("HOME") {
        if let Ok(rest) = p.strip_prefix(PathBuf::from(home)) {
            return if rest.as_os_str().is_empty() { "~".into() } else { format!("~/{}", rest.display()) };
        }
    }
    p.display().to_string()
}

fn check_folder_inner(repo: &str, dir: &Path) -> Result<String, String> {
    let want = normalize_repo(repo).ok_or(REPO_FORMAT_ERROR)?;
    if !dir.is_dir() {
        return Err(format!("Folder not found: {}", dir.display()));
    }
    let dir_s = dir.to_string_lossy();
    let url = match try_run("git", &["-C", &dir_s, "remote", "get-url", "origin"], CHECK_TIMEOUT) {
        Ok(url) => url,
        Err(e @ (RunError::NotInstalled | RunError::Timeout)) => return Err(e.message("git")),
        Err(RunError::Failed(_)) => return Err("This folder is not a git repo with an origin remote.".to_string()),
    };
    let got = repo_from_remote(&url).ok_or_else(|| format!("origin is not a GitHub repo: {}", redact_url(&url)))?;
    if !got.eq_ignore_ascii_case(&want) {
        return Err(format!("origin points to {got}, not {want}."));
    }
    // A subfolder of a clone has the same origin; the project folder must be the clone's top.
    let top = run("git", &["-C", &dir_s, "rev-parse", "--show-toplevel"])?;
    let top = PathBuf::from(top);
    if top.canonicalize().ok() != dir.canonicalize().ok() {
        return Err(format!("Pick the top folder of the clone: {}", with_tilde(&top)));
    }
    Ok(got)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{RwLock, RwLockReadGuard};

    /// Why this lock exists: a test that writes a script and then runs it can fail with
    /// "Text file busy" (ETXTBSY). Spawning a child with `pre_exec` makes std fork instead of
    /// `posix_spawn`, and between fork and exec the child still holds every open file
    /// descriptor of the parent, so a script that another test thread has open for writing
    /// is busy for any exec. The fix is mutual exclusion: a script is created, written and
    /// closed under the write lock (`write_script`), and every test that starts a process
    /// holds the read lock for its whole body (`spawning`). `Command::spawn` returns only
    /// after the exec succeeded, so no forked child outlives a read guard, and no writer can
    /// have a file open while any child is between fork and exec. Tests that only need a
    /// script to run use `sh <script>` and never exec the written file.
    static SPAWN_LOCK: RwLock<()> = RwLock::new(());

    fn spawning() -> RwLockReadGuard<'static, ()> {
        SPAWN_LOCK.read().unwrap_or_else(|e| e.into_inner())
    }

    /// Writes an executable script. Call it before `spawning()` (the lock is not re-entrant).
    fn write_script(path: &Path, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        let _w = SPAWN_LOCK.write().unwrap_or_else(|e| e.into_inner());
        std::fs::write(path, body).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn repo_input_forms() {
        assert_eq!(normalize_repo("a/b").as_deref(), Some("a/b"));
        assert_eq!(normalize_repo(" https://github.com/a-1/b.c.git/ ").as_deref(), Some("a-1/b.c"));
        assert_eq!(normalize_repo("github.com/a/b").as_deref(), Some("a/b"));
        assert_eq!(normalize_repo("https://github.com/a/b.git").as_deref(), Some("a/b"));
        assert_eq!(normalize_repo("https://www.github.com/a/b").as_deref(), Some("a/b"));
        assert_eq!(normalize_repo("a"), None);
        assert_eq!(normalize_repo("a/b/c"), None);
        assert_eq!(normalize_repo("a b/c"), None);
        assert_eq!(normalize_repo("https://gitlab.com/a/b"), None);
        assert_eq!(normalize_repo(""), None);
    }

    /// Resolver for tests: `work` is an alias of github.com, `other` of gitlab.com.
    fn fake_resolve(alias: &str) -> Option<String> {
        match alias {
            "github-work" => Some("github.com".into()),
            "gh-443" => Some("ssh.github.com".into()),
            "gl" => Some("gitlab.com".into()),
            _ => None,
        }
    }

    fn repo(url: &str) -> Option<String> {
        repo_from_remote_with(url, fake_resolve)
    }

    #[test]
    fn remote_url_forms() {
        assert_eq!(repo("git@github.com:o/n.git").as_deref(), Some("o/n"));
        assert_eq!(repo("git@github.com:o/n").as_deref(), Some("o/n"));
        assert_eq!(repo("https://github.com/o/n").as_deref(), Some("o/n"));
        assert_eq!(repo("https://github.com/o/n.git").as_deref(), Some("o/n"));
        assert_eq!(repo("http://github.com/o/n").as_deref(), Some("o/n"));
        assert_eq!(repo("ssh://git@github.com/o/n.git").as_deref(), Some("o/n"));
        assert_eq!(repo("https://www.github.com/o/n").as_deref(), Some("o/n"));
        assert_eq!(repo("  git@github.com:o/n.git\n").as_deref(), Some("o/n"));
    }

    #[test]
    fn remote_url_with_user_part() {
        assert_eq!(repo("https://user@github.com/o/n").as_deref(), Some("o/n"));
        assert_eq!(repo("https://x-access-token:ghp_TOKEN@github.com/o/n.git").as_deref(), Some("o/n"));
    }

    #[test]
    fn remote_url_with_port() {
        assert_eq!(repo("ssh://git@github.com:22/o/n").as_deref(), Some("o/n"));
        assert_eq!(repo("ssh://git@github.com:22/o/n.git").as_deref(), Some("o/n"));
        assert_eq!(repo("ssh://git@github.com:o/n"), None, "a non-numeric port is not a port");
    }

    #[test]
    fn remote_url_with_ssh_alias() {
        assert_eq!(repo("git@github-work:o/n.git").as_deref(), Some("o/n"));
        assert_eq!(repo("ssh://git@github-work/o/n.git").as_deref(), Some("o/n"));
        assert_eq!(repo("git@gh-443:o/n.git").as_deref(), Some("o/n"));
    }

    #[test]
    fn remote_url_rejects() {
        assert_eq!(repo("https://gitlab.com/o/n"), None);
        assert_eq!(repo("git@gitlab.com:o/n.git"), None);
        assert_eq!(repo("git@gl:o/n.git"), None, "alias that resolves to another host");
        assert_eq!(repo("git@unknown-alias:o/n.git"), None, "alias that does not resolve");
        assert_eq!(repo("https://github-work/o/n"), None, "no alias lookup for https");
        assert_eq!(repo("https://github.com.evil.com/o/n"), None);
        assert_eq!(repo("https://evil.com/github.com/o/n"), None);
        assert_eq!(repo("https://github.com/o"), None);
        assert_eq!(repo("https://github.com/o/n/extra"), None);
        assert_eq!(repo("ftp://github.com/o/n"), None);
        assert_eq!(repo("/srv/git/n.git"), None);
        assert_eq!(repo(""), None);
    }

    #[test]
    fn ssh_g_output_gives_the_hostname() {
        let out = "user git\nhostname github.com\nport 22\n";
        assert_eq!(hostname_from_ssh_g(out).as_deref(), Some("github.com"));
        assert_eq!(hostname_from_ssh_g("hostname GitHub.com\n").as_deref(), Some("github.com"));
        assert_eq!(hostname_from_ssh_g("user git\n"), None);
        assert_eq!(ssh_resolve_host("-oProxyCommand=evil"), None, "no option injection");
        assert_eq!(ssh_resolve_host("a b"), None);
        assert_eq!(ssh_resolve_host(""), None);
    }

    #[test]
    fn origin_url_is_redacted_for_messages() {
        assert_eq!(
            redact_url("https://x-access-token:ghp_SECRET@gitlab.com/o/n.git"),
            "https://gitlab.com/o/n.git"
        );
        assert_eq!(redact_url("https://user@gitlab.com/o/n"), "https://gitlab.com/o/n");
        assert_eq!(redact_url("https://gitlab.com/o/n"), "https://gitlab.com/o/n");
        assert_eq!(redact_url("git@gitlab.com:o/n.git"), "git@gitlab.com:o/n.git");
        assert_eq!(redact_url("https://gitlab.com/o@n"), "https://gitlab.com/o@n", "@ in the path stays");
    }

    #[test]
    fn marked_path_ignores_shell_noise() {
        let out = format!("Welcome!\n\x1b[0m{PATH_BEGIN}/a:/b{PATH_END}\nbye\n");
        assert_eq!(between_markers(&out), Some("/a:/b"));
        assert_eq!(between_markers("no markers"), None);
        assert_eq!(between_markers(&format!("{PATH_BEGIN}unterminated")), None);
        // An echoed command line holds the markers too; the last begin marker wins.
        let echoed = format!("printf '{PATH_BEGIN}' x\n{PATH_BEGIN}/real{PATH_END}");
        assert_eq!(between_markers(&echoed), Some("/real"));
    }

    #[test]
    fn a_slow_command_is_killed_at_the_timeout() {
        let _s = spawning();
        let start = Instant::now();
        let mut cmd = Command::new("sleep");
        cmd.arg("30");
        assert_eq!(stdout_with_timeout(cmd, Duration::from_millis(300), None), None);
        // Far below the 30 s sleep: the command was killed, not waited for.
        assert!(start.elapsed() < Duration::from_secs(10), "took {:?}", start.elapsed());
        let mut cmd = Command::new("printf");
        cmd.arg("hi");
        assert_eq!(stdout_with_timeout(cmd, Duration::from_secs(20), None).as_deref(), Some("hi"));
        assert_eq!(stdout_with_timeout(Command::new("/no/such/program"), Duration::from_secs(20), None), None);
    }

    #[test]
    fn the_shell_path_comes_through_markers_and_a_hanging_shell_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let noisy = dir.path().join("noisy-shell");
        // Stands in for an interactive shell: prints noise, then runs the script it was given.
        write_script(&noisy, "#!/bin/sh\necho 'banner from .zshrc'\nPATH=/x:/y eval \"$2\"\necho trailing\n");
        let hang = dir.path().join("hang-shell");
        write_script(&hang, "#!/bin/sh\nsleep 30\n");
        let _s = spawning();
        assert_eq!(shell_path(noisy.to_str().unwrap(), Duration::from_secs(20)).as_deref(), Some("/x:/y"));
        let start = Instant::now();
        assert_eq!(shell_path(hang.to_str().unwrap(), Duration::from_millis(300)), None);
        // Far below the 30 s sleep: the hanging shell was killed at the timeout.
        assert!(start.elapsed() < Duration::from_secs(10), "took {:?}", start.elapsed());
    }

    #[test]
    fn run_keeps_stdout_and_stderr_semantics() {
        let _s = spawning();
        assert_eq!(run("sh", &["-c", "echo out; echo err >&2"]).unwrap(), "out");
        assert_eq!(run("sh", &["-c", "echo err >&2"]).unwrap(), "err");
        assert_eq!(run("sh", &["-c", "echo first >&2; echo second >&2; exit 1"]).unwrap_err(), "first");
        assert_eq!(run("sh", &["-c", "echo only-out; exit 1"]).unwrap_err(), "only-out");
        assert!(run("no-such-program-xyz", &[]).unwrap_err().contains("not installed"));
    }

    #[test]
    fn a_hanging_check_command_fails_with_a_clear_message() {
        let _s = spawning();
        let start = Instant::now();
        let e = try_run("sleep", &["30"], Duration::from_millis(300)).unwrap_err();
        assert!(matches!(e, RunError::Timeout));
        assert!(start.elapsed() < Duration::from_secs(10), "took {:?}", start.elapsed());
        assert_eq!(e.message("gh"), "gh did not answer within 15 s.");
    }

    #[test]
    fn gh_errors_are_told_apart() {
        let signed_out = RunError::Failed("github.com\n  X Failed to log in to github.com account".into());
        assert!(gh_error_message(&signed_out).contains("gh auth login"));
        let none = RunError::Failed("You are not logged into any GitHub hosts. Run gh auth login to authenticate.".into());
        assert!(gh_error_message(&none).contains("gh auth login"));
        let net = RunError::Failed("error connecting to api.github.com\ncheck your internet connection".into());
        assert_eq!(gh_error_message(&net), "error connecting to api.github.com");
        assert_eq!(gh_error_message(&RunError::Timeout), "gh auth status did not answer within 15 s.");
        assert!(!gh_error_message(&RunError::Timeout).contains("signed in"));
    }

    #[test]
    fn the_path_probe_does_not_wait_for_a_background_process_that_holds_stdout() {
        let dir = tempfile::tempdir().unwrap();
        let shell = dir.path().join("bg-shell");
        // Like a .zshrc that starts a long-lived helper (gitstatusd, an agent) on stdout.
        write_script(&shell, "#!/bin/sh\n(sleep 30 &)\neval \"$2\"\n");
        let _s = spawning();
        let start = Instant::now();
        // The timeout is 20 s and the helper sleeps 30 s: a result before either means the
        // probe stopped at the end marker. (Waiting for EOF would give None at the timeout.)
        let got = shell_path(shell.to_str().unwrap(), Duration::from_secs(20));
        assert!(got.is_some_and(|p| !p.is_empty()));
        assert!(start.elapsed() < Duration::from_secs(15), "took {:?}", start.elapsed());
    }

    #[test]
    fn the_whole_process_group_is_killed() {
        let _s = spawning();
        // Gone = no such process. A killed child stays visible as a zombie until its parent
        // (init) reaps it, so poll instead of checking once.
        let gone = |pid: i32| {
            let deadline = Instant::now() + Duration::from_secs(10);
            while Instant::now() < deadline {
                if unsafe { kill(pid, 0) } == -1 {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            false
        };
        let read_pid = |f: &Path| -> i32 { std::fs::read_to_string(f).unwrap().trim().parse().unwrap() };
        let dir = tempfile::tempdir().unwrap();

        // End by timeout: the script writes its pid file, then waits for the sleep forever.
        let pidfile = dir.path().join("pid-timeout");
        let script = format!("sleep 60 & echo $! > {}; wait", pidfile.display());
        let mut cmd = Command::new("sh");
        cmd.args(["-c", &script]);
        assert!(stdout_with_timeout(cmd, Duration::from_secs(2), None).is_none());
        assert!(gone(read_pid(&pidfile)), "the background sleep must be gone after a timeout");

        // End early because the output is complete: the leftover helper is killed too.
        let pidfile = dir.path().join("pid-early");
        let script = format!("sleep 60 & echo $! > {}; echo ready; wait", pidfile.display());
        let mut cmd = Command::new("sh");
        cmd.args(["-c", &script]);
        let out = stdout_with_timeout(cmd, Duration::from_secs(20), Some(|o| o.contains("ready")));
        assert!(out.is_some_and(|o| o.contains("ready")));
        assert!(gone(read_pid(&pidfile)), "the background sleep must be gone after an early stop");
    }

    #[test]
    fn a_subfolder_of_a_clone_is_rejected() {
        let _s = spawning();
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_string_lossy().to_string();
        run("git", &["-C", &p, "init", "-q"]).unwrap();
        run("git", &["-C", &p, "remote", "add", "origin", "git@github.com:o/n.git"]).unwrap();
        let sub = dir.path().join("packages/web");
        std::fs::create_dir_all(&sub).unwrap();
        let e = check_folder_inner("o/n", &sub).unwrap_err();
        let top = dir.path().canonicalize().unwrap();
        assert!(e.starts_with("Pick the top folder of the clone: "), "{e}");
        assert!(e.contains(top.to_str().unwrap()) || e.contains("~/"), "{e}");
        assert!(check_folder_inner("o/n", dir.path()).is_ok());
    }

    #[test]
    fn tilde_form_of_a_path() {
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        assert_eq!(with_tilde(&home.join("code/x")), "~/code/x");
        assert_eq!(with_tilde(&home), "~");
        assert_eq!(with_tilde(Path::new("/opt/x")), "/opt/x");
    }

    #[test]
    fn waiting_for_the_path_lookup_is_bounded() {
        let never = (Mutex::new(false), Condvar::new());
        let start = Instant::now();
        assert!(!wait_ready(&never, Duration::from_millis(200)));
        assert!(start.elapsed() < Duration::from_secs(10), "took {:?}", start.elapsed());
        // A lookup that finishes while a check waits releases it early.
        let state = std::sync::Arc::new((Mutex::new(false), Condvar::new()));
        let s2 = state.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            mark_ready(&s2);
        });
        let start = Instant::now();
        // Released by the 100 ms thread long before the 30 s limit.
        assert!(wait_ready(&state, Duration::from_secs(30)));
        assert!(start.elapsed() < Duration::from_secs(15), "took {:?}", start.elapsed());
        // Already ready: no wait at all.
        assert!(wait_ready(&state, Duration::from_secs(30)));
    }

    #[test]
    fn folder_must_be_a_full_path() {
        assert_eq!(resolve_folder("code/shop-web").unwrap_err(), FULL_PATH_ERROR);
        assert_eq!(resolve_folder("./shop-web").unwrap_err(), FULL_PATH_ERROR);
        assert_eq!(resolve_folder("").unwrap_err(), FULL_PATH_ERROR);
        assert_eq!(resolve_folder("~other/x").unwrap_err(), FULL_PATH_ERROR);
        assert_eq!(resolve_folder("/tmp/x").unwrap(), PathBuf::from("/tmp/x"));
        let home = std::env::var("HOME").unwrap();
        assert_eq!(resolve_folder("~/code/x").unwrap(), PathBuf::from(home).join("code/x"));
        // The check reports the same text.
        let r = check_folder("o/n", "code/shop-web");
        assert!(!r.ok);
        assert_eq!(r.detail, FULL_PATH_ERROR);
    }

    #[test]
    fn folder_check_matches_origin() {
        let _s = spawning();
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_string_lossy().to_string();
        assert!(check_folder_inner("o/n", dir.path()).is_err(), "not a git repo yet");
        run("git", &["-C", &p, "init", "-q"]).unwrap();
        run("git", &["-C", &p, "remote", "add", "origin", "git@github.com:o/n.git"]).unwrap();
        assert_eq!(check_folder_inner("O/N", dir.path()).unwrap(), "o/n");
        assert!(check_folder_inner("o/other", dir.path()).unwrap_err().contains("points to o/n"));
        assert!(check_folder_inner("o/n", &dir.path().join("missing")).is_err());
        assert_eq!(check_folder_inner("nope", dir.path()).unwrap_err(), REPO_FORMAT_ERROR);
    }

    #[test]
    fn folder_check_never_prints_a_token() {
        let _s = spawning();
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_string_lossy().to_string();
        run("git", &["-C", &p, "init", "-q"]).unwrap();
        let url = "https://x-access-token:ghp_SECRET@gitlab.com/o/n.git";
        run("git", &["-C", &p, "remote", "add", "origin", url]).unwrap();
        let e = check_folder_inner("o/n", dir.path()).unwrap_err();
        assert!(e.contains("gitlab.com/o/n.git"), "{e}");
        assert!(!e.contains("ghp_SECRET") && !e.contains("x-access-token"), "{e}");
    }
}

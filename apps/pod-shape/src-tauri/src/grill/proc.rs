//! Child processes of the grill: each runs in its own process group (`setsid`, as in checks.rs), so Stop,
//! a timeout and app exit kill the whole group. The group id and the start time are stored in `runs`, so
//! the next app start can kill a group a crash left behind (advisor must-fix 3) without killing a reused
//! pid.

use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

extern "C" {
    fn setsid() -> i32;
    fn kill(pid: i32, sig: i32) -> i32;
}

pub const SIGTERM: i32 = 15;
pub const SIGKILL: i32 = 9;

/// Puts the command in a new session and process group (pid = pgid).
pub fn own_group(cmd: &mut Command) {
    // SAFETY: `setsid` is async-signal-safe, the only thing allowed between fork and exec.
    unsafe {
        cmd.pre_exec(|| {
            setsid();
            Ok(())
        });
    }
}

/// Signals a whole group. A no-op for pgid <= 1 (never signal init or "every process").
pub fn kill_group(pgid: i32, sig: i32) {
    if pgid > 1 {
        // SAFETY: plain syscall; a negative pid addresses the group.
        unsafe { kill(-pgid, sig) };
    }
}

/// True when any process of the group still exists.
pub fn group_alive(pgid: i32) -> bool {
    // SAFETY: signal 0 only checks existence.
    pgid > 1 && unsafe { kill(-pgid, 0) } == 0
}

/// SIGTERM, then SIGKILL after `grace` if the group is still there.
pub fn terminate_group(pgid: i32, grace: Duration) {
    kill_group(pgid, SIGTERM);
    let end = Instant::now() + grace;
    while Instant::now() < end {
        if !group_alive(pgid) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    kill_group(pgid, SIGKILL);
}

fn ps(pid: i32, field: &str) -> Option<String> {
    let out = Command::new("ps").args(["-o", &format!("{field}="), "-p", &pid.to_string()]).stdin(Stdio::null()).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !s.is_empty()).then_some(s)
}

/// `ps -o lstart=` of a pid (macOS and Linux), to tell a live group from a reused pid.
pub fn start_time(pid: i32) -> Option<String> {
    ps(pid, "lstart")
}

/// The command line of a pid.
pub fn command_line(pid: i32) -> Option<String> {
    ps(pid, "command")
}

/// A group left by a crash: kill it only when its leader still has the stored start time and runs
/// `claude` (or the test stand-in, whose path holds "claude"). Returns whether it killed something.
pub fn kill_orphan(pgid: i32, stored_start: Option<&str>) -> bool {
    let (Some(stored), Some(now)) = (stored_start, start_time(pgid)) else {
        return false;
    };
    if stored != now || !command_line(pgid).is_some_and(|c| c.contains("claude")) {
        return false;
    }
    terminate_group(pgid, Duration::from_secs(3));
    true
}

/// Reads a pipe to the end on a thread and keeps the last `cap` bytes.
pub fn tail_reader(mut pipe: impl Read + Send + 'static, cap: usize) -> (Arc<Mutex<Vec<u8>>>, std::thread::JoinHandle<()>) {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let b = buf.clone();
    let h = std::thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        while let Ok(n) = pipe.read(&mut chunk) {
            if n == 0 {
                break;
            }
            let mut g = b.lock().unwrap_or_else(|e| e.into_inner());
            g.extend_from_slice(&chunk[..n]);
            if g.len() > cap {
                let cut = g.len() - cap;
                g.drain(..cut);
            }
        }
    });
    (buf, h)
}

pub fn text_of(buf: &Mutex<Vec<u8>>) -> String {
    String::from_utf8_lossy(&buf.lock().unwrap_or_else(|e| e.into_inner())).into_owned()
}

/// A finished short command (git, gh, python3) of the ADR action.
#[derive(Debug, Clone)]
pub struct Output {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub ms: u128,
}

impl Output {
    pub fn ok(&self) -> bool {
        self.code == Some(0)
    }
    /// The last lines of stderr (else stdout), for an error row.
    pub fn error_tail(&self) -> String {
        let t = if self.stderr.trim().is_empty() { &self.stdout } else { &self.stderr };
        let lines: Vec<&str> = t.trim().lines().collect();
        lines[lines.len().saturating_sub(6)..].join("\n")
    }
}

/// Runs one command in its own group with a timeout; the group is killed at the end either way.
pub fn run(mut cmd: Command, cwd: Option<&Path>, timeout: Duration) -> std::io::Result<Output> {
    if let Some(c) = cwd {
        cmd.current_dir(c);
    }
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    own_group(&mut cmd);
    let t0 = Instant::now();
    let mut child: Child = cmd.spawn()?;
    let pgid = child.id() as i32;
    let (out, h1) = tail_reader(child.stdout.take().expect("piped"), 256 * 1024);
    let (err, h2) = tail_reader(child.stderr.take().expect("piped"), 64 * 1024);
    let deadline = t0 + timeout;
    let status = loop {
        if let Some(st) = child.try_wait()? {
            break Some(st);
        }
        if Instant::now() >= deadline {
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    kill_group(pgid, SIGKILL);
    let _ = child.wait();
    let _ = h1.join();
    let _ = h2.join();
    let mut stderr = text_of(&err);
    if status.is_none() {
        stderr.push_str(&format!("\nTimed out after {} s.", timeout.as_secs()));
    }
    Ok(Output { code: status.and_then(|s| s.code()), stdout: text_of(&out), stderr, ms: t0.elapsed().as_millis() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_captures_output_and_exit_code() {
        let mut c = Command::new("sh");
        c.args(["-c", "echo out; echo err >&2; exit 3"]);
        let o = run(c, None, Duration::from_secs(5)).unwrap();
        assert_eq!(o.code, Some(3));
        assert_eq!(o.stdout.trim(), "out");
        assert_eq!(o.error_tail(), "err");
    }

    #[test]
    fn run_kills_the_group_on_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let pidf = dir.path().join("pid");
        let mut c = Command::new("sh");
        c.args(["-c", &format!("sleep 30 & echo $! > {}; wait", pidf.display())]);
        let o = run(c, None, Duration::from_millis(300)).unwrap();
        assert_eq!(o.code, None);
        assert!(o.stderr.contains("Timed out"));
        let pid: i32 = std::fs::read_to_string(&pidf).unwrap().trim().parse().unwrap();
        std::thread::sleep(Duration::from_millis(100));
        // SAFETY: existence check only.
        assert_ne!(unsafe { kill(pid, 0) }, 0, "the background sleep survived");
    }

    #[test]
    fn kill_orphan_needs_the_same_start_time_and_a_claude_command() {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("claude");
        std::fs::write(&fake, "#!/bin/sh\nsleep 30\n").unwrap();
        std::fs::set_permissions(&fake, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let mut c = Command::new(&fake);
        own_group(&mut c);
        let mut child = c.spawn().unwrap();
        let pgid = child.id() as i32;
        std::thread::sleep(Duration::from_millis(100));
        let st = start_time(pgid).unwrap();
        assert!(!kill_orphan(pgid, Some("Mon Jan  1 00:00:00 2001")), "a different start time must not kill");
        assert!(!kill_orphan(pgid, None));
        assert!(group_alive(pgid));
        assert!(kill_orphan(pgid, Some(&st)));
        let _ = child.wait();
        assert!(!group_alive(pgid));
    }
}

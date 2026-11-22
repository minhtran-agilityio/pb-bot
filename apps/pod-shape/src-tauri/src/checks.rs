//! The three checks the Add project dialog runs before it saves a project:
//! `gh auth status`, `claude --version`, and the repo folder (`git -C <path> remote get-url origin`
//! must point at the GitHub repo the owner typed).

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub ok: bool,
    /// One line for the UI: the version, or what went wrong and how to fix it.
    pub detail: String,
    pub ms: u128,
}

/// An app started from Finder gets a short PATH (/usr/bin:/bin:...), so `gh` and `claude`
/// are not found. Take the PATH of the owner's login shell once at start, plus the usual
/// install folders.
pub fn fix_path_for_gui_app() {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let mut parts: Vec<String> = Vec::new();
    if let Ok(out) = Command::new(&shell).args(["-l", "-c", "printf %s \"$PATH\""]).output() {
        if out.status.success() {
            parts.extend(String::from_utf8_lossy(&out.stdout).split(':').map(String::from));
        }
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
    std::env::set_var("PATH", parts.join(":"));
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

/// Accepts `owner/name`, with an optional `https://github.com/` prefix or `.git` suffix.
pub fn normalize_repo(input: &str) -> Option<String> {
    let s = input.trim().trim_end_matches('/');
    let s = s
        .strip_prefix("https://github.com/")
        .or_else(|| s.strip_prefix("github.com/"))
        .unwrap_or(s);
    let s = s.strip_suffix(".git").unwrap_or(s);
    let mut it = s.split('/');
    let (owner, name) = (it.next()?, it.next()?);
    let valid = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    if it.next().is_some() || !valid(owner) || !valid(name) {
        return None;
    }
    Some(format!("{owner}/{name}"))
}

/// `git@github.com:o/n.git`, `https://github.com/o/n`, `ssh://git@github.com/o/n.git` -> `o/n`.
pub fn repo_from_remote(url: &str) -> Option<String> {
    let url = url.trim();
    let rest = url
        .strip_prefix("git@github.com:")
        .or_else(|| url.strip_prefix("ssh://git@github.com/"))
        .or_else(|| url.strip_prefix("https://github.com/"))
        .or_else(|| url.strip_prefix("http://github.com/"))?;
    normalize_repo(rest)
}

fn run(program: &str, args: &[&str]) -> Result<String, String> {
    match Command::new(program).args(args).output() {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(format!("{program} is not installed or not on PATH."))
        }
        Err(e) => Err(format!("{program} could not start: {e}")),
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if out.status.success() {
                Ok(if stdout.is_empty() { stderr } else { stdout })
            } else {
                let msg = if stderr.is_empty() { stdout } else { stderr };
                Err(msg.lines().next().unwrap_or("failed").to_string())
            }
        }
    }
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

pub fn check_gh() -> CheckResult {
    timed(|| {
        run("gh", &["auth", "status", "--hostname", "github.com"])
            .map(|_| "Signed in to github.com".to_string())
            .map_err(|e| {
                if e.contains("not installed") {
                    e
                } else {
                    "gh is not signed in. Run `gh auth login` in a terminal.".to_string()
                }
            })
    })
}

pub fn check_claude() -> CheckResult {
    timed(|| run("claude", &["--version"]).map(|v| v.lines().next().unwrap_or("").to_string()))
}

pub fn check_folder(repo: &str, path: &str) -> CheckResult {
    timed(|| check_folder_inner(repo, &expand_home(path)))
}

fn check_folder_inner(repo: &str, dir: &Path) -> Result<String, String> {
    let want = normalize_repo(repo).ok_or("Type the GitHub repo as owner/name.")?;
    if !dir.is_dir() {
        return Err(format!("Folder not found: {}", dir.display()));
    }
    let dir_s = dir.to_string_lossy();
    let url = run("git", &["-C", &dir_s, "remote", "get-url", "origin"])
        .map_err(|_| "This folder is not a git repo with an origin remote.".to_string())?;
    let got = repo_from_remote(&url).ok_or_else(|| format!("origin is not a GitHub repo: {url}"))?;
    if !got.eq_ignore_ascii_case(&want) {
        return Err(format!("origin points to {got}, not {want}."));
    }
    Ok(got)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_input_forms() {
        assert_eq!(normalize_repo("a/b").as_deref(), Some("a/b"));
        assert_eq!(normalize_repo(" https://github.com/a-1/b.c.git/ ").as_deref(), Some("a-1/b.c"));
        assert_eq!(normalize_repo("a"), None);
        assert_eq!(normalize_repo("a/b/c"), None);
        assert_eq!(normalize_repo("a b/c"), None);
    }

    #[test]
    fn remote_url_forms() {
        assert_eq!(repo_from_remote("git@github.com:o/n.git").as_deref(), Some("o/n"));
        assert_eq!(repo_from_remote("https://github.com/o/n").as_deref(), Some("o/n"));
        assert_eq!(repo_from_remote("ssh://git@github.com/o/n.git").as_deref(), Some("o/n"));
        assert_eq!(repo_from_remote("https://gitlab.com/o/n"), None);
    }

    #[test]
    fn folder_check_matches_origin() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_string_lossy().to_string();
        assert!(check_folder_inner("o/n", dir.path()).is_err(), "not a git repo yet");
        run("git", &["-C", &p, "init", "-q"]).unwrap();
        run("git", &["-C", &p, "remote", "add", "origin", "git@github.com:o/n.git"]).unwrap();
        assert_eq!(check_folder_inner("O/N", dir.path()).unwrap(), "o/n");
        assert!(check_folder_inner("o/other", dir.path()).unwrap_err().contains("points to o/n"));
        assert!(check_folder_inner("o/n", &dir.path().join("missing")).is_err());
    }
}

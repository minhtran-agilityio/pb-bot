//! The ADR step, run by the app (advisor must-fix 1; owner decision 3: git limited to `docs/adr/` plus
//! `gh pr create`, never merge). Claude only writes the decisions file. The app then, with argv it builds
//! itself, in two parts:
//! - `prepare`: fetch the default branch, make a worktree on `adr/<slug>` in a scratch folder, run
//!   pd-bot's `write_adrs.py`, and check that the change is only new `docs/adr/NNNN-*.md` files. The
//!   owner then reads the files and the pull request title in the app (security review finding 3:
//!   nothing Claude wrote reaches GitHub before the owner saw it).
//! - `publish` (after the owner's yes): commit those files, push only `refs/heads/adr/<slug>`, open a
//!   pull request into the default branch, remove the worktree. `discard` removes the worktree only.
//!
//! Every git call runs with the repo's hooks and fsmonitor off. The owner's checkout and branch are never
//! touched. This replaces the git part of pd-shape `references/adr.md` steps 2-3 (advisor-reviewed change).

use super::proc::{self, Output};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const GIT_TIMEOUT: Duration = Duration::from_secs(60);
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(30);

/// Where the commands come from. Tests point these at stand-ins; the app uses the PATH fix of checks.rs.
#[derive(Debug, Clone)]
pub struct Tools {
    pub git: String,
    pub gh: String,
    pub python: String,
    /// pd-bot's `scripts/write_adrs.py`.
    pub write_adrs: PathBuf,
}

impl Tools {
    pub fn find(home: &Path) -> Result<Tools, String> {
        // Test stand-ins only in a debug build; the release app always uses the real tools.
        let debug_env = |k: &str| if cfg!(debug_assertions) { std::env::var(k).ok() } else { None };
        let write_adrs = debug_env("SHAPE_BOARD_WRITE_ADRS")
            .map(PathBuf::from)
            .or_else(|| plugin_root(home).map(|r| r.join("scripts/write_adrs.py")).filter(|p| p.is_file()))
            .ok_or("pd-bot's write_adrs.py was not found under ~/.claude/plugins. Install or update the pd-bot plugin.")?;
        Ok(Tools { git: "git".into(), gh: debug_env("SHAPE_BOARD_GH").unwrap_or_else(|| "gh".into()), python: "python3".into(), write_adrs })
    }
}

/// The newest installed pd-bot plugin of the `pd-bot` marketplace: `~/.claude/plugins/cache/pd-bot/pd-bot/<version>`
/// (pinned, so another marketplace cannot supply the skill or the script). A version folder counts only when
/// it holds pd-shape.
pub fn plugin_root(home: &Path) -> Option<PathBuf> {
    let base = home.join(".claude/plugins/cache/pd-bot/pd-bot");
    std::fs::read_dir(base)
        .ok()?
        .flatten()
        .map(|v| v.path())
        .filter(|p| p.join("skills/pd-shape/SKILL.md").is_file())
        .max_by_key(|p| std::fs::metadata(p.join("skills/pd-shape/SKILL.md")).and_then(|m| m.modified()).ok())
}

/// One command that ran, for the RunLog.
#[derive(Debug, Clone)]
pub struct Step {
    pub display: String,
    pub argv: Vec<String>,
    pub out: Output,
}

/// A prepared ADR that waits for the owner: the worktree with the new files, not yet committed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prepared {
    pub repo: PathBuf,
    /// `owner/name` on GitHub, for `gh pr create --repo` (review L4: never the repo gh guesses).
    pub repo_name: String,
    pub worktree: PathBuf,
    pub branch: String,
    pub base: String,
    pub title: String,
    /// (path under the repo, text) of each new ADR file.
    pub files: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub files: Vec<String>,
    pub branch: String,
    pub pr_url: Option<String>,
}

/// `[a-z0-9-]`, at most 50 characters; `adr` when nothing is left.
pub fn safe_slug(s: &str) -> String {
    let slug = crate::artifacts::store::slugify(s);
    let slug: String = slug.chars().take(50).collect::<String>().trim_end_matches('-').to_string();
    if slug.is_empty() {
        "adr".into()
    } else {
        slug
    }
}

/// The run slug and the first decision title from the decisions file.
pub fn read_decisions(file: &Path) -> Result<(String, String), String> {
    let text = std::fs::read_to_string(file).map_err(|_| "Claude did not write the decisions file, so no ADR was made.".to_string())?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("The decisions file is not valid JSON: {e}"))?;
    let run = v.get("run").and_then(Value::as_str).unwrap_or("adr");
    let title = v
        .pointer("/decisions/0/title")
        .and_then(Value::as_str)
        .ok_or("The decisions file has no decision with a title.")?;
    Ok((safe_slug(run), title.chars().filter(|c| !c.is_control()).take(120).collect()))
}

/// Every line of `git status --porcelain --untracked-files=all` must be a new `docs/adr/NNNN-<slug>.md`.
pub fn validate_status(porcelain: &str) -> Result<Vec<String>, String> {
    let mut files = vec![];
    for line in porcelain.lines().filter(|l| !l.is_empty()) {
        let ok = line.strip_prefix("?? docs/adr/").is_some_and(|name| {
            let b = name.as_bytes();
            name.len() > 8
                && b[..4].iter().all(u8::is_ascii_digit)
                && b[4] == b'-'
                && name.ends_with(".md")
                && name[5..name.len() - 3].bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        });
        if !ok {
            return Err(format!("The ADR change touches more than new docs/adr files ({line}), so nothing was committed."));
        }
        files.push(line[3..].to_string());
    }
    Ok(files)
}

/// `user:token@` in a URL becomes `***@`, so a remote with a token never reaches the database.
pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("://") {
        out.push_str(&rest[..i + 3]);
        rest = &rest[i + 3..];
        let end = rest.find(|c: char| c.is_whitespace() || c == '/').unwrap_or(rest.len());
        if let Some(at) = rest[..end].rfind('@') {
            out.push_str("***");
            rest = &rest[at..];
        }
    }
    out.push_str(rest);
    out
}

fn cmd(program: &str, args: &[&str]) -> (Command, Vec<String>) {
    let mut c = crate::checks::command(program);
    let mut all: Vec<&str> = vec![];
    // The repo's hooks and fsmonitor never run for the app's own git calls (security review finding 4).
    if program == "git" || program.ends_with("/git") {
        all.extend(["-c", "core.hooksPath=/dev/null", "-c", "core.fsmonitor=false"]);
    }
    all.extend_from_slice(args);
    c.args(&all);
    // No prompt can hang a background git or gh.
    c.env("GIT_TERMINAL_PROMPT", "0").env("GH_PROMPT_DISABLED", "1");
    let argv = std::iter::once(program.to_string()).chain(all.iter().map(|s| s.to_string())).collect();
    (c, argv)
}

struct Runner<'a> {
    log: &'a mut dyn FnMut(&Step),
}

impl Runner<'_> {
    fn run(&mut self, display: &str, program: &str, args: &[&str], cwd: Option<&Path>, timeout: Duration) -> Result<Output, String> {
        let (c, argv) = cmd(program, args);
        let mut out = proc::run(c, cwd, timeout).map_err(|e| format!("{program} could not start: {e}"))?;
        out.stdout = redact(&out.stdout);
        out.stderr = redact(&out.stderr);
        (self.log)(&Step { display: display.to_string(), argv, out: out.clone() });
        Ok(out)
    }

    fn ok(&mut self, display: &str, program: &str, args: &[&str], cwd: Option<&Path>, timeout: Duration) -> Result<Output, String> {
        let out = self.run(display, program, args, cwd, timeout)?;
        if out.ok() {
            Ok(out)
        } else {
            Err(format!("{display} failed: {}", out.error_tail()))
        }
    }
}

/// The default branch: `origin/HEAD`, else `main`.
fn default_branch(r: &mut Runner, t: &Tools, repo: &str) -> String {
    r.run("git symbolic-ref origin/HEAD", &t.git, &["-C", repo, "symbolic-ref", "--short", "refs/remotes/origin/HEAD"], None, GIT_TIMEOUT)
        .ok()
        .filter(Output::ok)
        .and_then(|o| o.stdout.trim().strip_prefix("origin/").map(str::to_string))
        .filter(|b| !b.is_empty() && !b.starts_with('-'))
        .unwrap_or_else(|| "main".into())
}

/// Part 1: the worktree and the new ADR files, for the owner to read. `Ok(None)`: write_adrs.py made no
/// file (the decision did not meet the ADR rule); the worktree is already removed then.
pub fn prepare(
    repo: &Path,
    repo_name: &str,
    scratch: &Path,
    decisions_file: &Path,
    tools: &Tools,
    log: &mut dyn FnMut(&Step),
) -> Result<Option<Prepared>, String> {
    let (slug, title) = read_decisions(decisions_file)?;
    let mut r = Runner { log };
    let repo_s = repo.to_string_lossy().into_owned();
    let base = default_branch(&mut r, tools, &repo_s);
    r.ok(&format!("git fetch origin {base}"), &tools.git, &["-C", &repo_s, "fetch", "origin", &base], None, GIT_TIMEOUT)?;
    // A free branch name, here and on origin (review L3): adr/<slug>, adr/<slug>-2, ...
    let remote = r.ok(
        "git ls-remote origin adr/*",
        &tools.git,
        &["-C", &repo_s, "ls-remote", "--heads", "origin", "refs/heads/adr/*"],
        None,
        GIT_TIMEOUT,
    )?;
    let taken_remote: Vec<String> = remote.stdout.lines().filter_map(|l| l.split('\t').nth(1)).map(str::to_string).collect();
    let mut branch = format!("adr/{slug}");
    for i in 2..50 {
        let local = r
            .run("git rev-parse --verify", &tools.git, &["-C", &repo_s, "rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")], None, GIT_TIMEOUT)?
            .ok();
        if !local && !taken_remote.contains(&format!("refs/heads/{branch}")) {
            break;
        }
        branch = format!("adr/{slug}-{i}");
    }
    let wt: PathBuf = scratch.join(branch.replace('/', "-"));
    let wt_s = wt.to_string_lossy().into_owned();
    let start = format!("origin/{base}");
    r.ok(&format!("git worktree add -b {branch}"), &tools.git, &["-C", &repo_s, "worktree", "add", "-b", &branch, &wt_s, &start], None, GIT_TIMEOUT)?;
    let p = Prepared { repo: repo.to_path_buf(), repo_name: repo_name.to_string(), worktree: wt.clone(), branch: branch.clone(), base, title, files: vec![] };
    let result = (|| -> Result<Vec<(String, String)>, String> {
        let adr_dir = wt.join("docs/adr").to_string_lossy().into_owned();
        let script = tools.write_adrs.to_string_lossy().into_owned();
        let input = decisions_file.to_string_lossy().into_owned();
        r.ok("python3 write_adrs.py", &tools.python, &["-I", &script, "--input", &input, "--adr-dir", &adr_dir], Some(&wt), SCRIPT_TIMEOUT)?;
        let st = r.ok("git status", &tools.git, &["-C", &wt_s, "status", "--porcelain", "--untracked-files=all"], None, GIT_TIMEOUT)?;
        let mut files = vec![];
        for f in validate_status(&st.stdout)? {
            let text = std::fs::read_to_string(wt.join(&f)).map_err(|e| format!("{f} could not be read: {e}"))?;
            files.push((f, text));
        }
        Ok(files)
    })();
    match result {
        Ok(files) if !files.is_empty() => Ok(Some(Prepared { files, ..p })),
        other => {
            discard_with(&mut r, tools, &p);
            other.map(|_| None)
        }
    }
}

fn discard_with(r: &mut Runner, tools: &Tools, p: &Prepared) {
    let repo_s = p.repo.to_string_lossy().into_owned();
    let wt_s = p.worktree.to_string_lossy().into_owned();
    let _ = r.run("git worktree remove", &tools.git, &["-C", &repo_s, "worktree", "remove", "--force", &wt_s], None, GIT_TIMEOUT);
    let _ = r.run("git branch -D", &tools.git, &["-C", &repo_s, "branch", "-D", &p.branch], None, GIT_TIMEOUT);
}

/// The owner said no: remove the worktree and its local branch. Nothing was pushed.
pub fn discard(p: &Prepared, tools: &Tools, log: &mut dyn FnMut(&Step)) {
    discard_with(&mut Runner { log }, tools, p);
}

/// Part 2, after the owner read the files and said yes: commit exactly those files, push only the
/// `adr/<slug>` branch, open a pull request. Never merges. The worktree is removed either way.
pub fn publish(p: &Prepared, tools: &Tools, log: &mut dyn FnMut(&Step)) -> Result<Outcome, String> {
    let mut r = Runner { log };
    let wt_s = p.worktree.to_string_lossy().into_owned();
    let result = (|| -> Result<Outcome, String> {
        // The files must still be exactly the ones the owner read.
        let st = r.ok("git status", &tools.git, &["-C", &wt_s, "status", "--porcelain", "--untracked-files=all"], None, GIT_TIMEOUT)?;
        let now = validate_status(&st.stdout)?;
        let seen: Vec<&String> = p.files.iter().map(|(f, _)| f).collect();
        if now.iter().collect::<Vec<_>>() != seen {
            return Err("The ADR files changed after you read them, so nothing was pushed.".into());
        }
        for (f, text) in &p.files {
            if std::fs::read_to_string(p.worktree.join(f)).ok().as_deref() != Some(text.as_str()) {
                return Err("The ADR files changed after you read them, so nothing was pushed.".into());
            }
        }
        let mut add = vec!["-C", wt_s.as_str(), "add", "--"];
        add.extend(now.iter().map(String::as_str));
        r.ok("git add docs/adr", &tools.git, &add, None, GIT_TIMEOUT)?;
        let msg = format!("Add ADR: {}", p.title);
        r.ok("git commit", &tools.git, &["-C", &wt_s, "commit", "--no-verify", "-m", &msg], None, GIT_TIMEOUT)?;
        let refspec = format!("refs/heads/{0}:refs/heads/{0}", p.branch);
        r.ok(&format!("git push origin {}", p.branch), &tools.git, &["-C", &wt_s, "push", "origin", &refspec], None, GIT_TIMEOUT)?;
        let body = format!(
            "ADR from a Shape Board grill (the owner said yes in the app and read the file before this push).\n\nFiles:\n{}\n\nOpened by Shape Board. The owner reviews and merges this pull request.",
            now.iter().map(|f| format!("- `{f}`")).collect::<Vec<_>>().join("\n")
        );
        let pr_title = format!("ADR: {}", p.title);
        let pr = r.ok(
            "gh pr create",
            &tools.gh,
            &["pr", "create", "--repo", &p.repo_name, "--base", &p.base, "--head", &p.branch, "--title", &pr_title, "--body", &body],
            Some(&p.worktree),
            GIT_TIMEOUT,
        )?;
        let url = pr.stdout.lines().map(str::trim).find(|l| l.starts_with("https://")).map(str::to_string);
        Ok(Outcome { files: now, branch: p.branch.clone(), pr_url: url })
    })();
    let repo_s = p.repo.to_string_lossy().into_owned();
    let _ = r.run("git worktree remove", &tools.git, &["-C", &repo_s, "worktree", "remove", "--force", &wt_s], None, GIT_TIMEOUT);
    result
}

pub const NOTE_NO_FILE: &str = "ADR step: write_adrs.py made no ADR file (the decision did not meet the ADR rule). Nothing was pushed.";

/// The note shown in the chat and told to Claude in the next message.
pub fn note(result: &Result<Outcome, String>) -> String {
    match result {
        Ok(o) if o.files.is_empty() => NOTE_NO_FILE.into(),
        Ok(o) => format!(
            "ADR step done: {} on branch {}, pull request {}. The owner merges it.",
            o.files.join(", "),
            o.branch,
            o.pr_url.as_deref().unwrap_or("(no link printed)")
        ),
        Err(e) => format!("ADR step failed, nothing was merged: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn only_new_numbered_adr_files_pass() {
        assert_eq!(validate_status("").unwrap(), Vec::<String>::new());
        assert_eq!(validate_status("?? docs/adr/0007-phone-only.md\n").unwrap(), vec!["docs/adr/0007-phone-only.md"]);
        for bad in [
            " M docs/adr/0003-old.md",
            "?? docs/adr/TEMPLATE.md",
            "?? .github/workflows/x.yml",
            "?? docs/adr/0007-x.md\n?? docs/other.md",
            "?? docs/adr/0007-../../x.md",
            "?? docs/adr/7-x.md",
        ] {
            assert!(validate_status(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn slugs_are_safe() {
        assert_eq!(safe_slug("Phone sign-up; rm -rf /"), "phone-sign-up-rm-rf");
        assert_eq!(safe_slug("../.."), "adr");
        assert_eq!(safe_slug(&"a".repeat(80)).len(), 50);
    }

    fn git(dir: &Path, args: &[&str]) {
        let o = Command::new("git").current_dir(dir).args(args).output().unwrap();
        assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    }

    /// The whole step against a local bare remote, with a stand-in `gh` that prints a URL. No GitHub.
    #[test]
    fn the_app_runs_the_adr_step_against_a_local_bare_remote() {
        let d = tempfile::tempdir().unwrap();
        let remote = d.path().join("remote.git");
        let repo = d.path().join("repo");
        let scratch = d.path().join("scratch");
        fs::create_dir_all(&scratch).unwrap();
        git(d.path(), &["init", "-q", "--bare", "-b", "main", remote.to_str().unwrap()]);
        git(d.path(), &["clone", "-q", remote.to_str().unwrap(), repo.to_str().unwrap()]);
        fs::create_dir_all(repo.join("docs/adr")).unwrap();
        fs::write(repo.join("docs/adr/0001-first.md"), "# 1. First\n").unwrap();
        git(&repo, &["-c", "user.email=t@t", "-c", "user.name=t", "add", "."]);
        git(&repo, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", "init"]);
        git(&repo, &["push", "-q", "origin", "HEAD:main"]);
        git(&repo, &["remote", "set-head", "origin", "main"]);
        git(&repo, &["config", "user.email", "t@t"]);
        git(&repo, &["config", "user.name", "t"]);
        // The owner's checkout is on another branch with an uncommitted file: it must stay as it is.
        git(&repo, &["checkout", "-q", "-b", "owner-work"]);
        fs::write(repo.join("wip.txt"), "uncommitted").unwrap();

        let script = d.path().join("write_adrs.py");
        fs::write(
            &script,
            "import json,sys,os\nargs=sys.argv\nadr=args[args.index('--adr-dir')+1]\nos.makedirs(adr,exist_ok=True)\n\
             open(os.path.join(adr,'0002-phone-only.md'),'w').write('# 2. Phone only\\n')\nprint('[]')\n",
        )
        .unwrap();
        let gh = d.path().join("gh");
        fs::write(&gh, "#!/bin/sh\necho \"$@\" > \"$(dirname \"$0\")/gh-args\"\necho https://github.com/o/r/pull/9\n").unwrap();
        fs::set_permissions(&gh, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let decisions = scratch.join("decisions.json");
        fs::write(&decisions, r#"{"run": "Phone sign-up", "decisions": [{"title": "Phone only"}]}"#).unwrap();
        let tools = Tools { git: "git".into(), gh: gh.to_string_lossy().into(), python: "python3".into(), write_adrs: script };

        let mut steps = vec![];
        // Part 1: nothing is committed or pushed; the owner gets the file text.
        let p = prepare(&repo, "o/r", &scratch, &decisions, &tools, &mut |s| steps.push(s.argv.join(" "))).unwrap().unwrap();
        assert_eq!(p.branch, "adr/phone-sign-up");
        assert_eq!(p.files, vec![("docs/adr/0002-phone-only.md".to_string(), "# 2. Phone only\n".to_string())]);
        let remote_branches = || {
            let o = Command::new("git").args(["-C", remote.to_str().unwrap(), "branch", "--format=%(refname:short)"]).output().unwrap();
            String::from_utf8_lossy(&o.stdout).lines().map(str::to_string).collect::<Vec<_>>()
        };
        assert_eq!(remote_branches(), vec!["main"], "nothing pushed before the owner's yes");
        // Every git call runs with hooks and fsmonitor off.
        assert!(steps.iter().filter(|s| s.starts_with("git ")).all(|s| s.starts_with("git -c core.hooksPath=/dev/null -c core.fsmonitor=false ")), "{steps:?}");

        // Part 2: the owner said yes.
        let out = publish(&p, &tools, &mut |s| steps.push(s.argv.join(" "))).unwrap();
        assert_eq!(out.files, vec!["docs/adr/0002-phone-only.md"]);
        assert_eq!(out.pr_url.as_deref(), Some("https://github.com/o/r/pull/9"));
        assert_eq!(remote_branches(), vec!["adr/phone-sign-up", "main"]);
        let files = Command::new("git")
            .args(["-C", remote.to_str().unwrap(), "diff", "--name-only", "main", "adr/phone-sign-up"])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&files.stdout).trim(), "docs/adr/0002-phone-only.md");
        // The owner's checkout: same branch, the uncommitted file still there, the worktree gone.
        let head = Command::new("git").args(["-C", repo.to_str().unwrap(), "branch", "--show-current"]).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&head.stdout).trim(), "owner-work");
        assert_eq!(fs::read_to_string(repo.join("wip.txt")).unwrap(), "uncommitted");
        assert!(!p.worktree.exists());
        let gh_args = fs::read_to_string(d.path().join("gh-args")).unwrap();
        assert!(gh_args.starts_with("pr create --repo o/r --base main --head adr/phone-sign-up --title ADR: Phone only"), "{gh_args}");
        // No merge command of any kind (the PR body may say the owner "merges" it).
        assert!(!steps.iter().any(|s| s.split_whitespace().any(|w| w == "merge")), "{steps:?}");

        // The branch exists on origin but not here (deleted locally): the next ADR takes a new name (review L3).
        git(&repo, &["branch", "-D", "adr/phone-sign-up"]);
        let p2 = prepare(&repo, "o/r", &scratch, &decisions, &tools, &mut |_| {}).unwrap().unwrap();
        assert_eq!(p2.branch, "adr/phone-sign-up-2");
        discard(&p2, &tools, &mut |_| {});

        // A script that also edits an old ADR: refused in part 1, worktree and branch removed, nothing pushed.
        let bad = d.path().join("bad.py");
        fs::write(&bad, "import sys,os\nadr=sys.argv[sys.argv.index('--adr-dir')+1]\nopen(os.path.join(adr,'0001-first.md'),'a').write('superseded')\nopen(os.path.join(adr,'0003-x.md'),'w').write('x')\n").unwrap();
        let tools2 = Tools { write_adrs: bad, ..tools.clone() };
        let err = prepare(&repo, "o/r", &scratch, &decisions, &tools2, &mut |_| {}).unwrap_err();
        assert!(err.contains("more than new docs/adr files"), "{err}");
        assert_eq!(remote_branches().len(), 2);
        assert!(!scratch.join("adr-phone-sign-up-2").exists());

        // The owner said no: discard removes the worktree and the local branch; nothing pushed.
        let p3 = prepare(&repo, "o/r", &scratch, &decisions, &tools, &mut |_| {}).unwrap().unwrap();
        discard(&p3, &tools, &mut |_| {});
        assert!(!p3.worktree.exists());
        let b = Command::new("git").args(["-C", repo.to_str().unwrap(), "branch", "--list", &p3.branch]).output().unwrap();
        assert!(b.stdout.is_empty());
        assert_eq!(remote_branches().len(), 2);

        // A file changed after the owner read it: publish refuses.
        let p4 = prepare(&repo, "o/r", &scratch, &decisions, &tools, &mut |_| {}).unwrap().unwrap();
        fs::write(p4.worktree.join(&p4.files[0].0), "changed").unwrap();
        assert!(publish(&p4, &tools, &mut |_| {}).unwrap_err().contains("changed after you read them"));
        assert_eq!(remote_branches().len(), 2);
    }

    #[test]
    fn credentials_in_urls_are_redacted() {
        assert_eq!(redact("fatal: unable to access 'https://user:ghp_abc@github.com/o/r.git/'"), "fatal: unable to access 'https://***@github.com/o/r.git/'");
        assert_eq!(redact("https://github.com/o/r.git and ssh://git@host/x"), "https://github.com/o/r.git and ssh://***@host/x");
        assert_eq!(redact("no url"), "no url");
    }

    #[test]
    fn a_missing_decisions_file_makes_no_adr() {
        let d = tempfile::tempdir().unwrap();
        let tools = Tools { git: "git".into(), gh: "gh".into(), python: "python3".into(), write_adrs: d.path().join("x.py") };
        let e = prepare(d.path(), "o/r", d.path(), &d.path().join("none.json"), &tools, &mut |_| {}).unwrap_err();
        assert!(e.contains("did not write the decisions file"));
    }
}

#!/usr/bin/env python3
"""Generate or update the project wiki with Claude Code in print mode and an API key.

The model writes the pages; this script does everything else, with no model:

  1. guards    ANTHROPIC_API_KEY is set, the working tree is clean
  2. gate      `update` with nothing failed and nothing stale stops here (no model call, no cost)
  3. branch    git switch -c docs/wiki-<date>
  4. model     claude -p "/pd-bot:pd-wiki <mode> ..."   (the only step that uses the key)
  5. fence     every changed path must be inside the wiki folder
  6. check     wiki_check.py must print no FAIL line; if it does, no commit
  7. commit    on the branch, then wiki_check.py --strict (no stale page); with --pr also push
               and open a pull request

The script never merges and never writes to the default branch. When step 5 or 6 fails it stops
without a commit and leaves the files for you to read.

Exit: 0 done or nothing to do, 1 a guard, the fence or the check failed, 2 usage.
"""

from __future__ import annotations

import argparse
import datetime
import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
CHECK = HERE / "wiki_check.py"
PLUGIN = HERE.parent

PROMPT = (
    "/pd-bot:pd-wiki {mode}\n\n"
    "No person is present in this run. Do not ask a question. Do not commit, push, switch a "
    "branch or open a pull request: the caller does that. Change files only inside `{wiki}/`. "
    "When a step needs the owner's yes (a `.gitignore` line, the `CLAUDE.md` line), do not do it; "
    "list it in your report."
)

TOOLS = [
    "Read", "Write", "Edit", "Grep", "Glob",
    "Bash(git log *)", "Bash(git diff *)", "Bash(git show *)", "Bash(git ls-files *)",
    "Bash(git check-ignore *)", "Bash(python3 *wiki_check.py *)",
]


def git(root: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", "-C", str(root), *args], capture_output=True, text=True)


def changed_paths(root: Path) -> list[str]:
    out = git(root, "status", "--porcelain", "--untracked-files=all").stdout
    return [line[3:].split(" -> ")[-1].strip('"') for line in out.splitlines() if line.strip()]


def run_check(root: Path, wiki: str, strict: bool = True) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(CHECK), "--root", str(root), "--dir", wiki, *(["--strict"] if strict else [])],
        capture_output=True, text=True,
    )


def say(text: str) -> None:
    print(f"wiki-gen: {text}", flush=True)


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate or update the project wiki with an API key.")
    parser.add_argument("mode", nargs="?", choices=["init", "update"],
                        help="default: update when the wiki exists, else init")
    parser.add_argument("--root", default=".", help="repository root (default: the current directory)")
    parser.add_argument("--dir", default="docs/wiki", help="wiki folder, relative to the root")
    parser.add_argument("--model", default=None, help="model for the run (default: the Claude Code default)")
    parser.add_argument("--branch", default=None, help="branch to create (default: docs/wiki-<date>)")
    parser.add_argument("--claude", default="claude", help="the Claude Code command")
    parser.add_argument("--pr", action="store_true", help="push the branch and open a pull request")
    parser.add_argument("--dry-run", action="store_true", help="print the plan and the command; call nothing")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    wiki = args.dir.strip("/")
    if git(root, "rev-parse", "--is-inside-work-tree").returncode != 0:
        print(f"wiki-gen: not a git repository: {root}", file=sys.stderr)
        return 2
    mode = args.mode or ("update" if (root / wiki / "index.md").is_file() else "init")
    branch = args.branch or f"docs/wiki-{datetime.date.today().isoformat()}"
    command = [args.claude, "-p", PROMPT.format(mode=mode, wiki=wiki),
               "--plugin-dir", str(PLUGIN), "--permission-mode", "acceptEdits",
               "--allowedTools", *TOOLS]
    if args.model:
        command += ["--model", args.model]

    if args.dry_run:
        say(f"mode {mode}, branch {branch}, wiki folder {wiki}/")
        say("command: " + " ".join(command[:2]) + " '<prompt>' " + " ".join(command[3:]))
        return 0

    # 1. guards
    if not os.environ.get("ANTHROPIC_API_KEY"):
        say("ANTHROPIC_API_KEY is not set. Export it in this shell; never write it to a file in the repository.")
        return 1
    dirty = changed_paths(root)
    if dirty:
        say(f"the working tree is not clean ({len(dirty)} paths). Commit or stash first.")
        return 1

    # 2. gate
    if mode == "update":
        before = run_check(root, wiki)
        if before.returncode == 0:
            say("nothing failed and nothing is stale. No model call.")
            return 0
        print(before.stdout, end="")

    # 3. branch
    made = git(root, "switch", "-c", branch)
    if made.returncode != 0:
        say(f"cannot create the branch {branch}: {made.stderr.strip()}")
        return 1

    # 4. model
    say(f"calling the model (mode {mode}). This is the step that costs money.")
    ran = subprocess.run(command, cwd=root)
    if ran.returncode != 0:
        say(f"the model run stopped with exit {ran.returncode}. No commit. You are on the branch {branch}.")
        return 1

    # 5. fence
    changed = changed_paths(root)
    outside = [path for path in changed if not path.startswith(wiki + "/")]
    if outside:
        say("the model changed files outside the wiki. No commit. Read them, then `git restore` them:")
        for path in outside:
            print(f"  {path}")
        return 1
    if not changed:
        say("the model changed no file. No commit.")
        return 1

    # 6. check. A page stays stale until its change is committed, so the stale test runs after step 7.
    after = run_check(root, wiki, strict=False)
    if after.returncode != 0:
        print(after.stdout, end="")
        say("the check failed on the model's pages. No commit. Correct the pages or run again.")
        return 1

    # 7. commit
    git(root, "add", "--", wiki)
    done = git(root, "commit", "-m", f"docs(wiki): {mode} the project wiki\n\nWritten by wiki_gen.py ({mode}).")
    if done.returncode != 0:
        say(f"the commit failed: {done.stderr.strip() or done.stdout.strip()}")
        return 1
    say(f"committed {len(changed)} files on the branch {branch}.")
    after = run_check(root, wiki)
    print(after.stdout, end="")
    if after.returncode != 0:
        say("a page is still stale after the commit: the model did not examine it. Not pushed. Run again.")
        return 1
    if not args.pr:
        say(f"next: git push -u origin {branch}, then open a pull request. The owner merges it.")
        return 0
    pushed = git(root, "push", "-u", "origin", branch)
    if pushed.returncode != 0:
        say(f"the push failed: {pushed.stderr.strip()}")
        return 1
    opened = subprocess.run(
        ["gh", "pr", "create", "--head", branch, "--title", f"docs(wiki): {mode} the project wiki",
         "--body", "Wiki files only. Written by `wiki_gen.py`; `wiki_check.py --strict` passed.\n\n"
                   + after.stdout.strip().splitlines()[-1]],
        cwd=root, capture_output=True, text=True,
    )
    say(opened.stdout.strip() or opened.stderr.strip())
    return 0 if opened.returncode == 0 else 1


if __name__ == "__main__":
    sys.exit(main())

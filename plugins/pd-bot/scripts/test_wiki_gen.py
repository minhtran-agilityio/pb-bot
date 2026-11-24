"""Tests for wiki_gen.py. A fake `claude` command stands in for the model: no test calls the API."""

import os
import stat
import subprocess
import sys
from pathlib import Path

from test_wiki_check import git, make_repo

SCRIPT = Path(__file__).with_name("wiki_gen.py")
KEY = "sk-test-not-a-real-key"

# The fake model. FAKE_ACTION says what it writes; it records that it ran.
FAKE = """#!/usr/bin/env python3
import os, pathlib, sys
pathlib.Path(os.environ["FAKE_LOG"]).write_text("\\n".join(sys.argv[1:]))
action = os.environ.get("FAKE_ACTION", "good")
page = pathlib.Path("docs/wiki/rules.md")
if action == "good":
    page.write_text(page.read_text().replace("> Never merge on\\n> your own.", "> Merge when CI is green."))
elif action == "touch-index":
    index = pathlib.Path("docs/wiki/index.md")
    index.write_text(index.read_text() + chr(10))
elif action == "outside":
    pathlib.Path("src/rules.txt").write_text("changed by the model\\n")
elif action == "bad-quote":
    page.write_text(page.read_text().replace("> Never merge on\\n> your own.", "> Words that no source has."))
"""


def stale_repo(tmp_path: Path) -> Path:
    """A wiki whose quote no longer matches: `update` has work to do."""
    repo = make_repo(tmp_path)
    (repo / "src" / "rules.txt").write_text("Rule 1.\nMerge when CI is green.\n")
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", "change the rule")
    return repo


def run(repo: Path, tmp_path: Path, *args: str, action: str = "good", key: str | None = KEY):
    fake = tmp_path / "fake-claude"
    fake.write_text(FAKE)
    fake.chmod(fake.stat().st_mode | stat.S_IEXEC)
    env = {k: v for k, v in os.environ.items() if k != "ANTHROPIC_API_KEY"}
    env.update(FAKE_LOG=str(tmp_path / "fake.log"), FAKE_ACTION=action,
               GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@example.com",
               GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@example.com")
    if key:
        env["ANTHROPIC_API_KEY"] = key
    return subprocess.run(
        [sys.executable, str(SCRIPT), "--root", str(repo), "--claude", str(fake), "--branch", "docs/wiki-test", *args],
        capture_output=True, text=True, env=env,
    )


def model_ran(tmp_path: Path) -> bool:
    return (tmp_path / "fake.log").exists()


def test_no_key_stops_before_the_model(tmp_path):
    result = run(stale_repo(tmp_path), tmp_path, key=None)
    assert result.returncode == 1
    assert "ANTHROPIC_API_KEY is not set" in result.stdout
    assert not model_ran(tmp_path)


def test_dirty_tree_stops_before_the_model(tmp_path):
    repo = stale_repo(tmp_path)
    (repo / "notes.txt").write_text("work in progress\n")
    result = run(repo, tmp_path)
    assert result.returncode == 1
    assert "not clean" in result.stdout
    assert not model_ran(tmp_path)


def test_update_with_a_current_wiki_makes_no_model_call(tmp_path):
    result = run(make_repo(tmp_path), tmp_path)
    assert result.returncode == 0
    assert "No model call" in result.stdout
    assert not model_ran(tmp_path)


def test_good_run_commits_only_wiki_files_on_a_new_branch(tmp_path):
    repo = stale_repo(tmp_path)
    result = run(repo, tmp_path)
    assert result.returncode == 0, result.stdout + result.stderr
    assert git(repo, "branch", "--show-current") == "docs/wiki-test"
    assert git(repo, "show", "--name-only", "--format=", "HEAD").split() == ["docs/wiki/rules.md"]
    assert git(repo, "status", "--porcelain") == ""
    log = (tmp_path / "fake.log").read_text()
    assert "/pd-bot:pd-wiki update" in log and "--plugin-dir" in log
    assert "Do not commit, push" in log


def test_change_outside_the_wiki_is_not_committed(tmp_path):
    repo = stale_repo(tmp_path)
    head = git(repo, "rev-parse", "HEAD")
    result = run(repo, tmp_path, action="outside")
    assert result.returncode == 1
    assert "outside the wiki" in result.stdout and "src/rules.txt" in result.stdout
    assert git(repo, "rev-parse", "HEAD") == head


def test_failed_check_after_the_model_is_not_committed(tmp_path):
    repo = stale_repo(tmp_path)
    head = git(repo, "rev-parse", "HEAD")
    result = run(repo, tmp_path, action="bad-quote")
    assert result.returncode == 1
    assert "FAIL  quote" in result.stdout and "No commit" in result.stdout
    assert git(repo, "rev-parse", "HEAD") == head


def test_page_the_model_did_not_examine_stays_stale_and_is_not_pushed(tmp_path):
    repo = stale_repo(tmp_path)
    (repo / "docs" / "wiki" / "index.md").write_text((repo / "docs" / "wiki" / "index.md").read_text() + "\n")
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", "touch the index")
    (repo / "src" / "rules.txt").write_text("Rule 1.\nNever merge on your own. Ask.\n")
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", "the rule is back; the page is stale but its quote matches")
    result = run(repo, tmp_path, "--pr", action="touch-index")
    assert result.returncode == 1
    assert "still stale" in result.stdout and "Not pushed" in result.stdout


def test_the_key_is_never_printed_or_passed_as_an_argument(tmp_path):
    result = run(stale_repo(tmp_path), tmp_path)
    assert KEY not in result.stdout + result.stderr
    assert KEY not in (tmp_path / "fake.log").read_text()


def test_dry_run_calls_nothing_and_needs_no_key(tmp_path):
    result = run(stale_repo(tmp_path), tmp_path, "--dry-run", key=None)
    assert result.returncode == 0
    assert "mode update" in result.stdout
    assert not model_ran(tmp_path)

"""Tests for scripts/check-wiki.sh: the wiki check in run-tests.sh is report only (#146)."""

import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "plugins" / "pd-bot" / "scripts"))

from test_wiki_check import git, make_repo  # noqa: E402

SCRIPT = HERE / "check-wiki.sh"
RUN_TESTS = HERE / "run-tests.sh"


def run(repo: Path) -> subprocess.CompletedProcess:
    return subprocess.run(["bash", str(SCRIPT), str(repo)], capture_output=True, text=True)


def test_current_wiki_is_reported_ok(tmp_path):
    result = run(make_repo(tmp_path))
    assert result.returncode == 0
    assert "wiki (report only, see #146): ok" in result.stdout


def test_stale_quote_is_reported_but_does_not_fail(tmp_path):
    repo = make_repo(tmp_path)
    (repo / "src" / "rules.txt").write_text("Rule 1.\nMerge when CI is green.\n")
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", "change the rule")
    result = run(repo)
    assert "FAIL  quote" in result.stdout
    assert "wiki (report only, see #146)" in result.stdout
    assert "not a suite failure" in result.stdout
    assert result.returncode == 0


def test_new_adr_is_reported_but_does_not_fail(tmp_path):
    repo = make_repo(tmp_path)
    (repo / "docs" / "adr").mkdir(parents=True)
    (repo / "docs" / "adr" / "0007-demo.md").write_text("# 0007\n")
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", "add an ADR")
    result = run(repo)
    assert "FAIL  adr" in result.stdout
    assert result.returncode == 0


def test_run_tests_calls_the_report_only_script_and_keeps_the_unit_tests():
    text = RUN_TESTS.read_text()
    assert "bash scripts/check-wiki.sh" in text
    assert "wiki_check.py" not in text  # no direct call that could fail the suite
    assert "find scripts plugins/pd-bot -name 'test_*.py'" in text  # the unit tests of the wiki scripts still run

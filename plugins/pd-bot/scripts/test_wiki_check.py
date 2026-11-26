"""Tests for wiki_check.py. Each test builds a throwaway git repository."""

import json
import subprocess
import sys
from pathlib import Path

SCRIPT = Path(__file__).with_name("wiki_check.py")

PAGE = """---
title: Rules
read_when: before a merge
sources:
  - src/rules.txt
---

# Rules

> source: src/rules.txt
> Never merge on
> your own.
"""

INDEX = "# Wiki\n\n| Page | Read it when |\n|---|---|\n| [rules.md](rules.md) | before a merge |\n"


def git(repo: Path, *args: str) -> str:
    out = subprocess.run(
        ["git", "-C", str(repo), "-c", "user.name=t", "-c", "user.email=t@example.com", *args],
        capture_output=True, text=True, check=True,
    )
    return out.stdout.strip()


def make_repo(tmp_path: Path, page: str = PAGE, index: str = INDEX) -> Path:
    repo = tmp_path / "repo"
    (repo / "src").mkdir(parents=True)
    (repo / "docs" / "wiki").mkdir(parents=True)
    git(repo, "init", "-q")
    (repo / "src" / "rules.txt").write_text("Rule 1.\nNever   merge on your\nown. Ask first.\n")
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", "source")
    (repo / "docs" / "wiki" / "rules.md").write_text(page)
    (repo / "docs" / "wiki" / "index.md").write_text(index)
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", "wiki")
    return repo


def run(repo: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, str(SCRIPT), "--root", str(repo), *args],
                          capture_output=True, text=True)


def test_clean_wiki_passes_and_quote_ignores_line_breaks(tmp_path):
    result = run(make_repo(tmp_path))
    assert result.returncode == 0, result.stdout
    assert "1 pages, 1 quotes, 0 ADRs covered" in result.stdout
    assert "0 failed, 0 stale" in result.stdout


def test_changed_source_fails_the_quote(tmp_path):
    repo = make_repo(tmp_path)
    (repo / "src" / "rules.txt").write_text("Rule 1.\nMerge when CI is green.\n")
    result = run(repo)
    assert result.returncode == 1
    assert "FAIL  quote: docs/wiki/rules.md: not found in src/rules.txt" in result.stdout


def test_ref_reads_the_source_at_that_commit(tmp_path):
    repo = make_repo(tmp_path)
    (repo / "src" / "rules.txt").write_text("Rule 1.\nMerge when CI is green.\n")
    assert run(repo, "--ref", "HEAD").returncode == 0


def test_untracked_source_fails(tmp_path):
    repo = make_repo(tmp_path)
    (repo / ".gitignore").write_text("specs/\n")
    (repo / "specs").mkdir()
    (repo / "specs" / "plan.md").write_text("secret plan\n")
    page = repo / "docs" / "wiki" / "rules.md"
    page.write_text(page.read_text() + "\n> source: specs/plan.md\n> secret plan\n")
    result = run(repo)
    assert result.returncode == 1
    assert "quotes specs/plan.md, which git ignores (local only" in result.stdout


def test_link_to_an_ignored_file_fails(tmp_path):
    repo = make_repo(tmp_path)
    page = repo / "docs" / "wiki" / "rules.md"
    page.write_text(page.read_text() + "\nSee [the plan](../../specs/plan.md).\n")
    result = run(repo)
    assert result.returncode == 1
    assert "links to specs/plan.md, which git does not track" in result.stdout


def test_page_missing_from_index_and_without_quote_fails(tmp_path):
    repo = make_repo(tmp_path)
    extra = repo / "docs" / "wiki" / "extra.md"
    extra.write_text("---\ntitle: Extra\nread_when: never\nsources:\n  - src/rules.txt\n---\n\nNo quote.\n")
    git(repo, "add", "-A")
    result = run(repo)
    assert result.returncode == 1
    assert "docs/wiki/extra.md is not linked from index.md" in result.stdout
    assert "docs/wiki/extra.md has no quote block" in result.stdout


def test_size_limits(tmp_path):
    repo = make_repo(tmp_path)
    result = run(repo, "--max-page", "50", "--max-index", "10")
    assert result.returncode == 1
    assert "limit 50" in result.stdout and "limit 10" in result.stdout


def test_stale_page_is_reported_and_strict_fails(tmp_path):
    repo = make_repo(tmp_path)
    (repo / "src" / "rules.txt").write_text("Rule 0.\nNever merge on your own. Ask first.\n")
    git(repo, "commit", "-q", "-am", "edit the source")
    result = run(repo)
    assert result.returncode == 0
    assert "STALE docs/wiki/rules.md: changed after" in result.stdout
    assert run(repo, "--strict").returncode == 1


def test_golden_questions(tmp_path):
    repo = make_repo(tmp_path)
    golden = repo / "docs" / "wiki" / "golden.json"
    golden.write_text(json.dumps([{"q": "May I merge?", "page": "rules.md",
                                   "answer_contains": ["never merge on your own."]}]))
    assert run(repo).returncode == 0
    golden.write_text(json.dumps([{"q": "May I merge?", "page": "rules.md",
                                   "answer_contains": ["merge when CI is green"]}]))
    result = run(repo)
    assert result.returncode == 1
    assert 'rules.md lacks "merge when CI is green"' in result.stdout


def test_missing_index_fails(tmp_path):
    repo = make_repo(tmp_path)
    (repo / "docs" / "wiki" / "index.md").unlink()
    result = run(repo)
    assert result.returncode == 1
    assert "index.md is missing" in result.stdout


def test_link_into_an_ignored_folder_fails_even_when_the_file_exists(tmp_path):
    repo = make_repo(tmp_path)
    (repo / ".gitignore").write_text("/specs/\n")
    (repo / "specs" / "x").mkdir(parents=True)
    (repo / "specs" / "x" / "plan.md").write_text("plan\n")
    page = repo / "docs" / "wiki" / "rules.md"
    page.write_text(page.read_text() + "\nSee [the plan](../../specs/x/plan.md).\n")
    result = run(repo)
    assert result.returncode == 1
    assert "links to specs/x/plan.md, which git ignores (local only" in result.stdout


def test_new_adr_fails_until_a_page_names_it(tmp_path):
    repo = make_repo(tmp_path)
    (repo / "docs" / "adr").mkdir()
    (repo / "docs" / "adr" / "0001-first.md").write_text("# 0001. First\n\nWe chose A.\n")
    git(repo, "add", "-A")
    result = run(repo)
    assert result.returncode == 1
    assert "adr: docs/adr/0001-first.md is not a source of any wiki page" in result.stdout
    assert run(repo, "--adr-dir", "").returncode == 0
    page = repo / "docs" / "wiki" / "rules.md"
    page.write_text(page.read_text().replace("  - src/rules.txt\n", "  - src/rules.txt\n  - docs/adr/0001-first.md\n"))
    result = run(repo)
    assert result.returncode == 0, result.stdout
    assert "1 ADRs covered" in result.stdout


def test_golden_answer_contains_must_be_a_list_of_strings(tmp_path):
    repo = make_repo(tmp_path)
    golden = repo / "docs" / "wiki" / "golden.json"
    # A string used to be read one character at a time, so any page passed.
    golden.write_text(json.dumps([{"q": "May I merge?", "page": "rules.md",
                                   "answer_contains": "zzz not on the page"}]))
    result = run(repo)
    assert result.returncode == 1
    assert "`answer_contains` as a list of non-empty strings" in result.stdout
    golden.write_text(json.dumps([{"q": "May I merge?", "page": "rules.md", "answer_contains": [""]}]))
    assert run(repo).returncode == 1
    golden.write_text(json.dumps([{"q": "May I merge?", "page": "rules.md", "answer_contains": []}]))
    result = run(repo)
    assert result.returncode == 1
    assert "no `answer_contains`" in result.stdout


def test_golden_page_must_stay_inside_the_wiki_folder(tmp_path):
    repo = make_repo(tmp_path)
    (repo / "src" / "rules.txt").write_text("Rule 1.\nNever   merge on your\nown. Ask first.\n")
    golden = repo / "docs" / "wiki" / "golden.json"
    golden.write_text(json.dumps([{"q": "x", "page": "../../src/rules.txt", "answer_contains": ["rule 1"]}]))
    result = run(repo)
    assert result.returncode == 1
    assert "outside docs/wiki/" in result.stdout


def test_ref_that_starts_with_a_dash_is_refused(tmp_path):
    result = run(make_repo(tmp_path), "--ref=--output=/tmp/x")
    assert result.returncode == 2
    assert "must not start with '-'" in result.stderr

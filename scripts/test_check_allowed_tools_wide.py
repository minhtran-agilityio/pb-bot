"""Tests for scripts/check_allowed_tools_wide.py (issue #123) — run via pytest (wired into run-tests.sh).

The repro: `test_pd_shape_pre_allows_no_wide_rule` reads the real pd-shape SKILL.md. It fails on
main, where pd-shape pre-allowed `Bash(git -C *)`, `Bash(git push *)`, a bare `Read`, `Write` and
`Edit`, and more; it passes once pd-shape keeps only narrow rules.
"""

import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
CHECKER = HERE / "check_allowed_tools_wide.py"
REPO = HERE.parent
sys.path.insert(0, str(HERE))

import check_allowed_tools_wide as wide  # noqa: E402


def skill(tools: str) -> str:
    return f"---\nname: alpha\ndescription: d\nallowed-tools: {tools}\n---\n\nbody\n"


def build(root: Path, skills: dict[str, str]) -> None:
    for name, tools in skills.items():
        (root / "skills" / name).mkdir(parents=True, exist_ok=True)
        (root / "skills" / name / "SKILL.md").write_text(skill(tools))


def run(root: Path, baseline: dict | None = None) -> subprocess.CompletedProcess:
    path = root / "baseline.json"
    path.write_text(json.dumps(baseline or {}))
    return subprocess.run(
        [sys.executable, str(CHECKER), "--root", str(root), "--baseline", str(path), "--base-ref", ""],
        capture_output=True,
        text=True,
    )


# ── the repro: the real pd-shape ─────────────────────────────────────────────────────────


def test_pd_shape_pre_allows_no_wide_rule():
    text = (REPO / "plugins/pd-bot/skills/pd-shape/SKILL.md").read_text()
    found = [(e, r) for e in wide.entries(wide.frontmatter(text)) if (r := wide.why_wide(e))]
    assert found == [], f"pd-shape pre-allows wide rules: {found}"


def test_live_tree_passes_with_its_baseline():
    result = subprocess.run([sys.executable, str(CHECKER)], capture_output=True, text=True)
    assert result.returncode == 0, result.stderr


def test_baseline_never_names_pd_shape():
    baseline = json.loads((HERE / "allowed-tools-wide-baseline.json").read_text())
    assert "pd-shape" not in baseline


# ── wide shapes from the issue and the probes: each must be flagged ──────────────────────

WIDE = [
    "Bash",
    "Bash(git *)",
    "Bash(git -C *)",
    "Bash(git log *)",
    "Bash(git fetch *)",
    "Bash(git push *)",
    "Bash(git push*)",
    "Bash(git commit *)",
    "Bash(git worktree *)",
    "Bash(git add docs/adr*)",
    "Bash(python3 *)",
    "Bash(python3 -c*)",
    "Bash(bash -c *)",
    "Bash(env *)",
    "Bash(gh pr create *)",
    "Bash(gh issue comment *)",
    "Bash(gh issue *)",
    "Bash(gh api *)",
    "Bash(* --version)",
    "Bash(bash scripts/run-tests.sh*)",
    "Read",
    "Write",
    "Edit",
    "Grep",
    "Glob",
    "Read(**)",
    "Read(//**)",
    "Edit(~/**)",
    # from the security review of this change
    "Read(~/.claude/plugins/**)",
    "Bash(tmux send-keys *)",
    "Bash(find *)",
    "Bash(env git *)",
    "Bash(/usr/bin/git *)",
    "Bash(gh auth status *)",
    "Bash(python3 -m pkg *)",
    "Bash (git *)",
    "Read(~/.ssh/**)",
    "Read(../**)",
    "Read(./../x/**)",
    "Read(/home/**)",
    "Edit(./**)",
    "Edit(./.git/**)",
    "Edit(./.claude/**)",
    # from the reviewer: writes that reach CI, hooks or harness scripts
    "Edit(./.github/**)",
    "Edit(./.github/workflows/ci.yml)",
    "Edit(./scripts/**)",
    "Edit(./plugins/**)",
    "Edit(./plugins/pd-bot/hooks/**)",
    "Write(./plugins/pd-bot/scripts/x.sh)",
    "Edit(./*.json)",
]

NARROW = [
    "Agent",
    "AskUserQuestion",
    "Bash(gh issue view *)",
    "Bash(gh pr view *)",
    "Bash(gh pr list *)",
    "Bash(ls *)",
    "Bash(bash scripts/run-tests.sh)",
    "Bash(python3 ${CLAUDE_PLUGIN_ROOT}/scripts/write_adrs.py *)",
    "Bash(bash ${CLAUDE_PLUGIN_ROOT}/scripts/fleet-check.sh *)",
    "Bash(git status)",
    "Read(./**)",
    "Read(~/.claude/plugins/cache/*/pd-bot/**)",
    "Edit(./specs/**)",
    "Edit(./docs/adr/**)",
    "Edit(./plugins/pd-bot/skills/x/**)",
    "Grep(./**)",
    "Bash(bash ${CLAUDE_SKILL_DIR}/scripts/publish-check.sh *)",
    "Bash(test -n *)",
    "Bash(gh api user --jq .login)",
]


def test_every_wide_shape_is_flagged():
    missed = [e for e in WIDE if wide.why_wide(e) is None]
    assert missed == []


def test_narrow_rules_are_not_flagged():
    flagged = [(e, wide.why_wide(e)) for e in NARROW if wide.why_wide(e)]
    assert flagged == []


def test_comma_inside_parentheses_does_not_split():
    block = "allowed-tools: Read(./**), Bash(echo a, b), Agent"
    assert wide.entries(block) == ["Read(./**)", "Bash(echo a, b)", "Agent"]


# ── the gate: baseline behaviour ─────────────────────────────────────────────────────────


def test_new_wide_rule_fails(tmp_path):
    build(tmp_path, {"alpha": "Read(./**), Bash(git push *)"})
    result = run(tmp_path)
    assert result.returncode == 1
    assert "git push" in result.stderr


def test_baselined_wide_rule_passes(tmp_path):
    build(tmp_path, {"alpha": "Read(./**), Bash(git *)"})
    assert run(tmp_path, {"alpha": ["Bash(git *)"]}).returncode == 0


def test_stale_baseline_entry_fails(tmp_path):
    build(tmp_path, {"alpha": "Read(./**)"})
    result = run(tmp_path, {"alpha": ["Bash(git *)"]})
    assert result.returncode == 1
    assert "is gone" in result.stderr


def test_never_wide_skill_cannot_be_baselined(tmp_path):
    build(tmp_path, {"pd-shape": "Read(./**), Bash(git -C *)"})
    result = run(tmp_path, {"pd-shape": ["Bash(git -C *)"]})
    assert result.returncode == 1
    assert "NEVER_WIDE" in result.stderr


def test_no_skills_fails_closed(tmp_path):
    (tmp_path / "skills").mkdir()
    assert run(tmp_path).returncode == 2


def test_quoted_value_fails_closed():
    assert wide.why_wide(wide.entries('allowed-tools: "Bash, Read"')[0])


def test_continued_value_fails_closed():
    block = "allowed-tools: Read(./**),\n  Bash\nname: x"
    assert [e for e in wide.entries(block) if wide.why_wide(e)]


def test_repeated_key_fails_closed():
    block = "allowed-tools: Read(./**)\nallowed-tools: Bash"
    assert [e for e in wide.entries(block) if wide.why_wide(e)]


def test_baseline_entry_missing_on_the_base_ref_fails(tmp_path):
    """The baseline only shrinks: an entry the base ref does not list fails, even when added
    in the same change as the wide rule."""
    git = ["git", "-C", str(tmp_path), "-c", "user.email=t@t", "-c", "user.name=t"]
    subprocess.run([*git, "init", "-q", "-b", "main"], check=True)
    build(tmp_path, {"alpha": "Read(./**), Bash(git *)"})
    (tmp_path / "baseline.json").write_text(json.dumps({"alpha": ["Bash(git *)"]}))
    subprocess.run([*git, "add", "-A"], check=True)
    subprocess.run([*git, "commit", "-q", "-m", "base"], check=True)
    build(tmp_path, {"alpha": "Read(./**), Bash(git *), Bash(tmux send-keys *)"})
    grown = {"alpha": ["Bash(git *)", "Bash(tmux send-keys *)"]}
    (tmp_path / "baseline.json").write_text(json.dumps(grown))
    result = subprocess.run(
        [sys.executable, str(CHECKER), "--root", str(tmp_path),
         "--baseline", str(tmp_path / "baseline.json"), "--base-ref", "main"],
        capture_output=True, text=True,
    )
    assert result.returncode == 1
    assert "only shrinks" in result.stderr


def _checker(*argv, cwd=None):
    return subprocess.run([sys.executable, str(CHECKER), *argv], capture_output=True, text=True, cwd=cwd)


def test_shrink_check_fails_closed_when_the_base_ref_is_missing(tmp_path):
    git = ["git", "-C", str(tmp_path), "-c", "user.email=t@t", "-c", "user.name=t"]
    subprocess.run([*git, "init", "-q", "-b", "main"], check=True)
    build(tmp_path, {"alpha": "Read(./**)"})
    (tmp_path / "baseline.json").write_text("{}")
    subprocess.run([*git, "add", "-A"], check=True)
    subprocess.run([*git, "commit", "-q", "-m", "base"], check=True)
    result = _checker("--root", str(tmp_path), "--baseline", str(tmp_path / "baseline.json"),
                      "--base-ref", "no-such-ref")
    assert result.returncode == 1
    assert "does not resolve" in result.stderr


def test_shrink_check_fails_closed_outside_a_git_repository(tmp_path):
    build(tmp_path, {"alpha": "Read(./**)"})
    (tmp_path / "baseline.json").write_text("{}")
    result = _checker("--root", str(tmp_path), "--baseline", str(tmp_path / "baseline.json"),
                      "--base-ref", "main")
    assert result.returncode == 1
    assert "shrink check cannot run" in result.stderr


def test_base_ref_without_a_baseline_file_passes_with_a_note(tmp_path):
    """The change that adds the baseline: the ref exists but has no baseline file yet."""
    git = ["git", "-C", str(tmp_path), "-c", "user.email=t@t", "-c", "user.name=t"]
    subprocess.run([*git, "init", "-q", "-b", "main"], check=True)
    build(tmp_path, {"alpha": "Read(./**)"})
    subprocess.run([*git, "add", "-A"], check=True)
    subprocess.run([*git, "commit", "-q", "-m", "base"], check=True)
    (tmp_path / "baseline.json").write_text("{}")
    result = _checker("--root", str(tmp_path), "--baseline", str(tmp_path / "baseline.json"),
                      "--base-ref", "main")
    assert result.returncode == 0, result.stderr
    assert "has no baseline yet" in result.stdout

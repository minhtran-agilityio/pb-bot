"""Tests for fleet-settings.py: each case uses its own throwaway git checkout."""
import json
import os
import subprocess
import sys

import pytest

SCRIPT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "fleet-settings.py")


# A global excludes file (common: **/.claude/settings.local.json) would hide the "not ignored" case,
# so git runs without the global and system config.
GIT_ENV = {**os.environ, "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1",
           "XDG_CONFIG_HOME": os.devnull}


def run(*args):
    return subprocess.run([sys.executable, SCRIPT, *args], capture_output=True, text=True, env=GIT_ENV)


@pytest.fixture
def checkout(tmp_path):
    d = tmp_path / "co"
    d.mkdir()
    subprocess.run(["git", "init", "-q", str(d)], check=True)
    (d / ".gitignore").write_text(".claude/settings.local.json\n")
    return d


def settings(d):
    return d / ".claude" / "settings.local.json"


def state(d):
    return d / ".git" / "pd-bot-fleet-settings.json"


def apply(d, *extra):
    return run("--checkout", str(d), "--role", "worker", "--model", "claude-haiku-5-5",
               "--mode", "auto", *extra)


def test_writes_model_mode_and_role(checkout):
    r = apply(checkout, "--ticket", "12", "--run", "r1")
    assert r.returncode == 0, r.stderr
    data = json.loads(settings(checkout).read_text())
    assert data["model"] == "claude-haiku-5-5"
    assert data["permissions"]["defaultMode"] == "auto"
    assert data["env"] == {"PD_BOT_ROLE": "worker", "PD_BOT_TICKET": "12", "PD_BOT_RUN": "r1"}


def test_keeps_other_keys_and_restores_the_file(checkout):
    settings(checkout).parent.mkdir()
    before = {"permissions": {"allow": ["Bash(npm test)"], "defaultMode": "manual"},
              "env": {"FOO": "1"}, "model": "claude-opus-5-5"}
    text = json.dumps(before)
    settings(checkout).write_text(text)
    assert apply(checkout).returncode == 0
    data = json.loads(settings(checkout).read_text())
    assert data["permissions"]["allow"] == ["Bash(npm test)"]
    assert data["env"]["FOO"] == "1" and data["env"]["PD_BOT_ROLE"] == "worker"
    r = run("--checkout", str(checkout), "--restore")
    assert r.returncode == 0, r.stderr
    assert json.loads(settings(checkout).read_text()) == before
    assert not state(checkout).exists()


def test_nothing_is_written_in_the_work_tree_but_the_settings_file(checkout):
    assert apply(checkout).returncode == 0
    assert state(checkout).exists()
    st = subprocess.run(["git", "-C", str(checkout), "status", "--porcelain", "--ignored"],
                        capture_output=True, text=True, env=GIT_ENV).stdout
    assert "pd-bot-backup" not in st
    assert [l for l in st.splitlines() if "!!" not in l] == ["?? .gitignore"]


def test_restore_keeps_a_rule_the_session_saved(checkout):
    assert apply(checkout).returncode == 0
    data = json.loads(settings(checkout).read_text())
    data["permissions"]["allow"] = ["Bash(npm run e2e)"]
    settings(checkout).write_text(json.dumps(data))
    assert run("--checkout", str(checkout), "--restore").returncode == 0
    assert json.loads(settings(checkout).read_text()) == {"permissions": {"allow": ["Bash(npm run e2e)"]}}


def test_a_refused_apply_writes_nothing(checkout):
    settings(checkout).parent.mkdir()
    settings(checkout).write_text('{"permissions": []}')
    assert apply(checkout).returncode == 1
    assert not state(checkout).exists()
    settings(checkout).write_text('{"permissions": {}}')
    assert apply(checkout).returncode == 0
    assert run("--checkout", str(checkout), "--restore").returncode == 0
    assert json.loads(settings(checkout).read_text()) == {}


def test_a_file_that_looks_like_an_old_marker_is_restored(checkout):
    settings(checkout).parent.mkdir()
    settings(checkout).write_text('{"pd-bot-backup": "no file before"}\n')
    assert apply(checkout).returncode == 0
    assert run("--checkout", str(checkout), "--restore").returncode == 0
    assert json.loads(settings(checkout).read_text()) == {"pd-bot-backup": "no file before"}


def test_not_a_git_checkout_is_named(tmp_path):
    d = tmp_path / "plain"
    d.mkdir()
    r = apply(d)
    assert r.returncode == 1 and "not a git checkout" in r.stderr


def test_bedrock_and_vertex_model_ids_are_accepted(checkout):
    for m in ("us.anthropic.claude-opus-5-5-v1:0", "claude-opus-5-5@20261101"):
        r = run("--checkout", str(checkout), "--role", "worker", "--model", m, "--mode", "auto")
        assert r.returncode == 0, r.stderr


def test_restore_removes_a_file_that_was_not_there(checkout):
    assert apply(checkout).returncode == 0
    assert run("--checkout", str(checkout), "--restore").returncode == 0
    assert not settings(checkout).exists()


def test_a_second_write_keeps_the_first_backup(checkout):
    settings(checkout).parent.mkdir()
    settings(checkout).write_text('{"model": "x"}')
    assert apply(checkout).returncode == 0
    assert run("--checkout", str(checkout), "--role", "reviewer", "--model", "claude-opus-5-5",
               "--mode", "auto").returncode == 0
    assert json.loads(settings(checkout).read_text())["env"]["PD_BOT_ROLE"] == "reviewer"
    assert "PD_BOT_TICKET" not in json.loads(settings(checkout).read_text())["env"]
    run("--checkout", str(checkout), "--restore")
    assert json.loads(settings(checkout).read_text()) == {"model": "x"}


def test_restore_without_backup_is_a_no_op(checkout):
    r = run("--checkout", str(checkout), "--restore")
    assert r.returncode == 0 and "nothing to restore" in r.stdout, r.stderr


def test_refuses_a_file_git_does_not_ignore(checkout):
    (checkout / ".gitignore").write_text("")
    r = apply(checkout)
    assert r.returncode == 1 and "not ignored by git" in r.stderr
    assert not settings(checkout).exists()


def test_refuses_bypass_permissions(checkout):
    r = run("--checkout", str(checkout), "--role", "worker", "--model", "m", "--mode", "bypassPermissions")
    assert r.returncode == 2


def test_refuses_a_symlinked_settings_file(checkout, tmp_path):
    settings(checkout).parent.mkdir()
    target = tmp_path / "elsewhere.json"
    target.write_text("{}")
    os.symlink(target, settings(checkout))
    r = apply(checkout)
    assert r.returncode == 1 and "symlink" in r.stderr
    assert target.read_text() == "{}"


def test_refuses_a_symlinked_claude_dir(checkout, tmp_path):
    other = tmp_path / "other"
    other.mkdir()
    os.symlink(other, checkout / ".claude")
    r = apply(checkout)
    assert r.returncode == 1 and "symlink" in r.stderr


def test_refuses_invalid_json(checkout):
    settings(checkout).parent.mkdir()
    settings(checkout).write_text("{nope")
    r = apply(checkout)
    assert r.returncode == 1 and "not valid JSON" in r.stderr
    assert settings(checkout).read_text() == "{nope"


def test_refuses_a_non_object_env(checkout):
    settings(checkout).parent.mkdir()
    settings(checkout).write_text('{"env": ["x"]}')
    r = apply(checkout)
    assert r.returncode == 1 and "env is not an object" in r.stderr


@pytest.mark.parametrize("args", [
    ["--role", "worker", "--mode", "auto"],
    ["--role", "boss", "--model", "m", "--mode", "auto"],
    ["--role", "worker", "--model", "m; rm -rf x", "--mode", "auto"],
    ["--role", "worker", "--model", "m", "--mode", "auto", "--ticket", "1 2"],
    ["--restore", "--role", "worker"],
])
def test_usage_errors_exit_2(checkout, args):
    assert run("--checkout", str(checkout), *args).returncode == 2


def test_missing_checkout_exits_2(tmp_path):
    assert run("--checkout", str(tmp_path / "none"), "--restore").returncode == 2


def show(d):
    return run("--checkout", str(d), "--show")


def test_show_prints_absent_keys_and_no_restore_state(checkout):
    r = show(checkout)
    assert r.returncode == 0, r.stderr
    assert r.stdout.splitlines() == [
        "model: absent", "permissions.defaultMode: absent", "env.PD_BOT_ROLE: absent",
        "env.PD_BOT_TICKET: absent", "env.PD_BOT_RUN: absent", "restore state: none"]
    assert not settings(checkout).parent.exists() and not state(checkout).exists()


def test_show_prints_values_and_pending_restore_and_writes_nothing(checkout):
    assert apply(checkout, "--ticket", "12", "--run", "r1").returncode == 0
    before = (settings(checkout).read_text(), state(checkout).read_text())
    r = show(checkout)
    assert r.returncode == 0, r.stderr
    assert r.stdout.splitlines() == [
        'model: "claude-haiku-5-5"', 'permissions.defaultMode: "auto"',
        'env.PD_BOT_ROLE: "worker"', 'env.PD_BOT_TICKET: "12"', 'env.PD_BOT_RUN: "r1"',
        "restore state: present"]
    assert (settings(checkout).read_text(), state(checkout).read_text()) == before


def test_show_reports_a_partial_file_without_restore_state(checkout):
    settings(checkout).parent.mkdir()
    settings(checkout).write_text('{"model": "m", "env": {"FOO": "1"}}')
    out = show(checkout).stdout.splitlines()
    assert out[0] == 'model: "m"' and out[2] == "env.PD_BOT_ROLE: absent"
    assert out[-1] == "restore state: none"


def test_show_refuses_invalid_json_and_leaves_it(checkout):
    settings(checkout).parent.mkdir()
    settings(checkout).write_text("{nope")
    r = show(checkout)
    assert r.returncode == 1 and "not valid JSON" in r.stderr
    assert settings(checkout).read_text() == "{nope"


@pytest.mark.parametrize("extra", [
    ["--restore"], ["--role", "worker"], ["--model", "m"], ["--mode", "auto"],
    ["--ticket", "1"], ["--run", "r"], ["--ticket", ""], ["--run", ""], ["--model", ""],
    ["--role", "worker", "--model", "m", "--mode", "auto"]])
def test_show_cannot_be_combined(checkout, extra):
    assert apply(checkout).returncode == 0
    before = settings(checkout).read_text()
    r = run("--checkout", str(checkout), "--show", *extra)
    assert r.returncode == 2 and "--show takes only --checkout" in r.stderr
    assert settings(checkout).read_text() == before and state(checkout).exists()

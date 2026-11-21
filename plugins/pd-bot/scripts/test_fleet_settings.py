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
    assert settings(checkout).read_text() == text
    assert not os.path.exists(str(settings(checkout)) + ".pd-bot-backup")


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
    assert settings(checkout).read_text() == '{"model": "x"}'


def test_restore_without_backup_is_a_no_op(checkout):
    r = run("--checkout", str(checkout), "--restore")
    assert r.returncode == 0 and "nothing to restore" in r.stdout


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

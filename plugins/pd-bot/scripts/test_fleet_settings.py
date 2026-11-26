"""Tests for fleet-settings.py: each case uses its own throwaway git checkout."""
import json
import os
import subprocess
import sys

import pytest

SCRIPT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "fleet-settings.py")
with open(os.path.join(os.path.dirname(SCRIPT), "fleet-trim.json")) as _f:
    TRIM = json.load(_f)
_SCALARS = sorted(k for k in TRIM if k not in ("disableClaudeAiConnectors", "enabledPlugins"))
# `--show` always ends the fleet keys with autoCompactWindow; the trim file does not set it (#180).
TRIM_LINES = ["disableClaudeAiConnectors: " + json.dumps(TRIM["disableClaudeAiConnectors"])] + [
    f"{k}: {json.dumps(TRIM[k])}" for k in _SCALARS] + [
    f"enabledPlugins.{n}: false" for n in sorted(TRIM["enabledPlugins"])] + ["autoCompactWindow: absent"]
TRIM_ABSENT = ["disableClaudeAiConnectors: absent"] + [f"{k}: absent" for k in _SCALARS] + [
    f"enabledPlugins.{n}: absent" for n in sorted(TRIM["enabledPlugins"])] + ["autoCompactWindow: absent"]


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
    assert data["disableClaudeAiConnectors"] is True
    # Replaced the old assertion that the default was TRIM["autoCompactWindow"] (500000), per #180.
    assert "autoCompactWindow" not in data and "autoCompactWindow" not in TRIM
    assert data["enabledPlugins"] == {n: False for n in TRIM["enabledPlugins"]}


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
        r = run("--checkout", str(checkout), "--role", "worker", "--model", m, "--mode", "auto",
                "--depth", "L")
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
    ["--show", "--restore"],
    ["--show", "--role", "worker"],
    ["--show", "--model", "m"],
    ["--show", "--mode", "auto"],
    ["--show", "--ticket", "1"],
    ["--show", "--run", "r1"],
    ["--show", "--ticket="],
    ["--show", "--run="],
    ["--show", "--model="],
    ["--show", "--depth", "L"],
    ["--show", "--escalated"],
    ["--show", "--effort", "high"],
    ["--restore", "--effort", "high"],
    ["--role", "worker", "--model", "m", "--mode", "auto", "--effort", "max"],
    ["--restore", "--keep-connectors"],
    ["--role", "worker", "--model", "m", "--mode", "auto", "--depth", "XL"],
])
def test_usage_errors_exit_2(checkout, args):
    assert run("--checkout", str(checkout), *args).returncode == 2


def test_missing_options_error_names_show_and_restore(checkout):
    r = run("--checkout", str(checkout))
    assert r.returncode == 2
    err = [l for l in r.stderr.splitlines() if "are required" in l]
    assert len(err) == 1 and "--restore" in err[0] and "--show" in err[0]


def test_missing_checkout_exits_2(tmp_path):
    assert run("--checkout", str(tmp_path / "none"), "--restore").returncode == 2


def show(d):
    return run("--checkout", str(d), "--show")


def test_show_prints_absent_keys_and_no_restore_state(checkout):
    r = show(checkout)
    assert r.returncode == 0, r.stderr
    assert r.stdout.splitlines() == [
        "model: absent", "permissions.defaultMode: absent", "env.PD_BOT_ROLE: absent",
        "env.PD_BOT_TICKET: absent", "env.PD_BOT_RUN: absent", "effortLevel: absent", *TRIM_ABSENT,
        "restore state: none"]
    assert not (checkout / ".claude").exists() and not state(checkout).exists()


def test_show_prints_values_after_apply_and_writes_nothing(checkout):
    assert apply(checkout, "--ticket", "12", "--run", "r1").returncode == 0
    before = (settings(checkout).read_text(), state(checkout).read_text())
    r = show(checkout)
    assert r.returncode == 0, r.stderr
    assert r.stdout.splitlines() == [
        'model: "claude-haiku-5-5"', 'permissions.defaultMode: "auto"', 'env.PD_BOT_ROLE: "worker"',
        'env.PD_BOT_TICKET: "12"', 'env.PD_BOT_RUN: "r1"', "effortLevel: absent", *TRIM_LINES,
        "restore state: present"]
    assert (settings(checkout).read_text(), state(checkout).read_text()) == before


def test_show_reports_no_restore_state_after_restore(checkout):
    assert apply(checkout).returncode == 0
    assert run("--checkout", str(checkout), "--restore").returncode == 0
    assert show(checkout).stdout.splitlines()[-1] == "restore state: none"


def test_show_leaves_other_keys_out_and_refuses_bad_json(checkout):
    settings(checkout).parent.mkdir()
    settings(checkout).write_text('{"permissions": {"allow": ["x"]}, "env": {"FOO": "1"}}')
    out = show(checkout).stdout
    assert "FOO" not in out and "allow" not in out
    settings(checkout).write_text("{nope")
    r = show(checkout)
    assert r.returncode == 1 and "not valid JSON" in r.stderr
    assert settings(checkout).read_text() == "{nope"


def test_show_works_when_git_does_not_ignore_the_file(checkout):
    (checkout / ".gitignore").write_text("")
    assert show(checkout).returncode == 0


def test_show_reports_an_invalid_restore_state_file_and_writes_nothing(checkout):
    assert apply(checkout).returncode == 0
    state(checkout).write_text("{nope")
    before = (settings(checkout).read_text(), state(checkout).read_text())
    r = show(checkout)
    assert r.returncode == 0, r.stderr
    assert r.stdout.splitlines()[-1] == "restore state: present (invalid)"
    assert (settings(checkout).read_text(), state(checkout).read_text()) == before
    assert run("--checkout", str(checkout), "--restore").returncode == 1


def test_show_reports_a_non_object_restore_state_as_invalid_and_restore_refuses_it(checkout):
    assert apply(checkout).returncode == 0
    state(checkout).write_text("[]")
    r = show(checkout)
    assert r.returncode == 0, r.stderr
    assert r.stdout.splitlines()[-1] == "restore state: present (invalid)"
    r = run("--checkout", str(checkout), "--restore")
    assert r.returncode == 1 and "Traceback" not in r.stderr


def test_show_exits_0_and_reports_invalid_when_the_restore_state_path_is_a_directory(checkout):
    state(checkout).mkdir()
    r = show(checkout)
    assert r.returncode == 0, r.stderr
    assert r.stdout.splitlines()[-1] == "restore state: present (invalid)"


@pytest.mark.parametrize("model", ["claude-opus-5-5", "claude-fable-5-2", "opus", "best", "default",
                                   "opusplan", "us.anthropic.claude-opus-5-5-v1:0"])
def test_a_worker_on_opus_or_fable_is_refused_below_depth_l(checkout, model):
    for extra in ([], ["--depth", "S"], ["--depth", "M"]):
        r = run("--checkout", str(checkout), "--role", "worker", "--model", model, "--mode", "auto", *extra)
        assert r.returncode == 1 and "worker runs Sonnet" in r.stderr, (extra, r.stderr)
    assert not settings(checkout).exists() and not state(checkout).exists()


@pytest.mark.parametrize("extra", [["--depth", "L"], ["--escalated"], ["--depth", "M", "--escalated"]])
def test_a_worker_on_opus_is_allowed_at_depth_l_or_escalated(checkout, extra):
    r = run("--checkout", str(checkout), "--role", "worker", "--model", "claude-opus-5-5",
            "--mode", "auto", *extra)
    assert r.returncode == 0, r.stderr


def test_other_roles_and_sonnet_workers_are_not_guarded(checkout):
    for role, model in (("reviewer", "claude-opus-5-5"), ("advisor", "claude-fable-5-2"),
                        ("worker", "claude-sonnet-5-5"), ("worker", "sonnet"), ("worker", "haiku"),
                        ("worker", "us.anthropic.claude-sonnet-5-5-v1:0")):
        r = run("--checkout", str(checkout), "--role", role, "--model", model, "--mode", "auto")
        assert r.returncode == 0, r.stderr


def test_keep_connectors_writes_no_connector_trim_and_no_compact_window(checkout):
    assert apply(checkout, "--keep-connectors").returncode == 0
    data = json.loads(settings(checkout).read_text())
    assert "disableClaudeAiConnectors" not in data and "enabledPlugins" not in data
    assert "autoCompactWindow" not in data


def test_apply_and_restore_leave_an_existing_compact_window_alone(checkout):
    settings(checkout).parent.mkdir()
    before = {"autoCompactWindow": 650000}
    settings(checkout).write_text(json.dumps(before))
    assert apply(checkout).returncode == 0
    assert json.loads(settings(checkout).read_text())["autoCompactWindow"] == 650000
    assert "autoCompactWindow" not in state(checkout).read_text()
    assert run("--checkout", str(checkout), "--restore").returncode == 0
    assert json.loads(settings(checkout).read_text()) == before


def test_a_run_without_the_flag_leaves_no_compact_window_in_the_workspace_settings(checkout):
    assert apply(checkout, "--ticket", "1", "--run", "r").returncode == 0
    assert "autoCompactWindow" not in json.loads(settings(checkout).read_text())
    assert run("--checkout", str(checkout), "--restore").returncode == 0
    assert not settings(checkout).exists()


def test_a_cap_in_the_trim_file_is_never_copied(checkout, tmp_path, monkeypatch):
    import importlib.util
    spec = importlib.util.spec_from_file_location("fleet_settings", SCRIPT)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    trim = tmp_path / "trim.json"
    trim.write_text(json.dumps({"autoCompactWindow": 500000, "someOtherKey": 1,
                                "enabledPlugins": {"x@m": False}}))
    monkeypatch.setattr(mod, "TRIM_FILE", str(trim))
    keys = [k for k, _ in mod.trim_values()]
    assert ("autoCompactWindow",) not in keys and ("someOtherKey",) in keys


def test_compact_window_flag_writes_the_value_and_restore_puts_the_owner_value_back(checkout):
    settings(checkout).parent.mkdir()
    before = {"autoCompactWindow": 777000, "model": "x"}
    settings(checkout).write_text(json.dumps(before))
    r = apply(checkout, "--compact-window", "400000")
    assert r.returncode == 0, r.stderr
    assert json.loads(settings(checkout).read_text())["autoCompactWindow"] == 400000
    assert "autoCompactWindow: 400000" in show(checkout).stdout
    assert run("--checkout", str(checkout), "--restore").returncode == 0
    assert json.loads(settings(checkout).read_text()) == before


def test_compact_window_flag_with_no_owner_value_is_removed_by_restore(checkout):
    assert apply(checkout, "--compact-window", "250000", "--keep-connectors").returncode == 0
    assert json.loads(settings(checkout).read_text())["autoCompactWindow"] == 250000
    assert run("--checkout", str(checkout), "--restore").returncode == 0
    assert not settings(checkout).exists()


def test_a_later_apply_without_the_flag_puts_the_owner_window_back(checkout):
    settings(checkout).parent.mkdir()
    settings(checkout).write_text('{"autoCompactWindow": 777000}')
    assert apply(checkout, "--compact-window", "300000").returncode == 0
    assert apply(checkout).returncode == 0
    assert json.loads(settings(checkout).read_text())["autoCompactWindow"] == 777000


@pytest.mark.parametrize("n", ["99999", "1000000000", "abc", "-5", ""])
def test_compact_window_refuses_a_bad_number_and_writes_nothing(checkout, n):
    r = apply(checkout, "--compact-window", n)
    assert r.returncode == 2 and "Traceback" not in r.stderr, r.stderr
    assert not settings(checkout).exists() and not state(checkout).exists()


def test_show_rejects_compact_window(checkout):
    assert run("--checkout", str(checkout), "--show", "--compact-window", "300000").returncode == 2


def test_restore_puts_back_the_owner_plugin_choices(checkout):
    settings(checkout).parent.mkdir()
    name = sorted(TRIM["enabledPlugins"])[0]
    before = {"enabledPlugins": {name: True, "mine@market": True}, "disableClaudeAiConnectors": False}
    settings(checkout).write_text(json.dumps(before))
    assert apply(checkout).returncode == 0
    data = json.loads(settings(checkout).read_text())
    assert data["enabledPlugins"][name] is False and data["enabledPlugins"]["mine@market"] is True
    assert data["disableClaudeAiConnectors"] is True
    assert run("--checkout", str(checkout), "--restore").returncode == 0
    assert json.loads(settings(checkout).read_text()) == before


def test_a_second_apply_adds_the_trim_the_first_skipped(checkout):
    settings(checkout).parent.mkdir()
    settings(checkout).write_text('{"model": "x"}')
    assert apply(checkout, "--keep-connectors").returncode == 0
    assert apply(checkout).returncode == 0
    assert json.loads(settings(checkout).read_text())["disableClaudeAiConnectors"] is True
    assert run("--checkout", str(checkout), "--restore").returncode == 0
    assert json.loads(settings(checkout).read_text()) == {"model": "x"}


def test_repo_project_settings_carry_the_plugin_trim_and_keep_only_claude_docs():
    repo = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(SCRIPT))))
    with open(os.path.join(repo, ".claude", "settings.json")) as f:
        proj = json.load(f)
    assert {k: v for k, v in proj["enabledPlugins"].items() if k in TRIM["enabledPlugins"]} \
        == TRIM["enabledPlugins"]
    # The owner's sessions keep the Claude Docs connector; every other claude.ai connector is denied.
    deny = proj["permissions"]["deny"]
    assert "disableClaudeAiConnectors" not in proj
    # No default cap on the context window (#180): neither the project settings nor the trim set one.
    assert "autoCompactWindow" not in proj
    assert all(r.startswith("mcp__claude_ai_") for r in deny)
    assert "mcp__claude_ai_Claude_Docs" not in deny and "mcp__claude_ai_Context7" not in deny


def test_keep_connectors_after_a_trimmed_apply_puts_the_owner_values_back(checkout):
    settings(checkout).parent.mkdir()
    settings(checkout).write_text('{"enabledPlugins": {"mine@m": true}}')
    assert apply(checkout).returncode == 0
    assert json.loads(settings(checkout).read_text())["disableClaudeAiConnectors"] is True
    assert apply(checkout, "--keep-connectors").returncode == 0
    data = json.loads(settings(checkout).read_text())
    assert "disableClaudeAiConnectors" not in data and data["enabledPlugins"] == {"mine@m": True}
    assert "autoCompactWindow" not in data
    assert run("--checkout", str(checkout), "--restore").returncode == 0
    assert json.loads(settings(checkout).read_text()) == {"enabledPlugins": {"mine@m": True}}


@pytest.mark.parametrize("body", ["{nope", "[]", '{"extra": {}}', '{"extra": [{"path": [["x"]]}]}',
                                  '{"extra": [{"path": []}]}', '{"extra": ["x"]}'])
def test_a_bad_restore_state_is_refused_without_a_traceback(checkout, body):
    state(checkout).write_text(body)
    r = apply(checkout)
    assert r.returncode == 1 and "Traceback" not in r.stderr and "restore state" in r.stderr, r.stderr


def test_a_restore_state_that_is_a_directory_is_refused(checkout):
    state(checkout).mkdir()
    r = apply(checkout)
    assert r.returncode == 1 and "Traceback" not in r.stderr, r.stderr


def test_effort_is_written_and_restored(checkout):
    settings(checkout).parent.mkdir()
    settings(checkout).write_text('{"effortLevel": "low"}')
    assert apply(checkout, "--effort", "high").returncode == 0
    assert json.loads(settings(checkout).read_text())["effortLevel"] == "high"
    assert run("--checkout", str(checkout), "--restore").returncode == 0
    assert json.loads(settings(checkout).read_text()) == {"effortLevel": "low"}


def test_no_effort_option_removes_an_earlier_apply_effort(checkout):
    assert apply(checkout, "--effort", "high").returncode == 0
    assert apply(checkout).returncode == 0
    assert "effortLevel" not in json.loads(settings(checkout).read_text())


def test_keeper_is_a_role_on_its_own(checkout):
    r = run("--checkout", str(checkout), "--role", "keeper", "--model", "claude-sonnet-5-5",
            "--mode", "auto", "--effort", "high")
    assert r.returncode == 0, r.stderr
    data = json.loads(settings(checkout).read_text())
    assert data["env"]["PD_BOT_ROLE"] == "keeper" and data["effortLevel"] == "high"

"""Tests for ledger-backup.py: each case uses its own throwaway git checkout."""
import json
import os
import subprocess
import sys

import pytest

SCRIPT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "ledger-backup.py")
GIT_ENV = {**os.environ, "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1"}


def run(*args):
    return subprocess.run([sys.executable, SCRIPT, *args], capture_output=True, text=True, env=GIT_ENV)


@pytest.fixture
def checkout(tmp_path):
    d = tmp_path / "co"
    d.mkdir()
    subprocess.run(["git", "init", "-q", str(d)], check=True)
    return d


def ledger(d, run_id="r1"):
    return d / "specs" / "fleet" / run_id / "ledger.json"


def backup(d, run_id="r1"):
    return d / ".git" / "pd-bot" / "fleet" / run_id / "ledger.json"


def write_ledger(d, data, run_id="r1"):
    ledger(d, run_id).parent.mkdir(parents=True, exist_ok=True)
    ledger(d, run_id).write_text(json.dumps(data))


def test_save_copies_the_ledger_into_the_git_dir(checkout):
    write_ledger(checkout, {"v": 1, "tickets": {"1": {"next": "wait review-result"}}})
    r = run("--checkout", str(checkout), "--run", "r1")
    assert r.returncode == 0, r.stderr
    assert json.loads(backup(checkout).read_text())["tickets"]["1"]["next"] == "wait review-result"


def test_the_backup_survives_git_clean_and_restores(checkout):
    write_ledger(checkout, {"v": 1, "run": "r1"})
    assert run("--checkout", str(checkout), "--run", "r1").returncode == 0
    subprocess.run(["git", "-C", str(checkout), "clean", "-fdxq"], check=True, env=GIT_ENV)
    assert not ledger(checkout).exists() and backup(checkout).exists()
    r = run("--checkout", str(checkout), "--run", "r1", "--restore")
    assert r.returncode == 0, r.stderr
    assert json.loads(ledger(checkout).read_text()) == {"v": 1, "run": "r1"}


def test_restore_never_overwrites_an_existing_ledger(checkout):
    write_ledger(checkout, {"v": 1, "old": True})
    assert run("--checkout", str(checkout), "--run", "r1").returncode == 0
    write_ledger(checkout, {"v": 1, "new": True})
    r = run("--checkout", str(checkout), "--run", "r1", "--restore")
    assert r.returncode == 1 and "not overwriting" in r.stderr
    assert json.loads(ledger(checkout).read_text()) == {"v": 1, "new": True}


@pytest.mark.parametrize("body", ["{nope", "[]"])
def test_a_bad_ledger_is_refused_and_the_old_backup_kept(checkout, body):
    write_ledger(checkout, {"v": 1})
    assert run("--checkout", str(checkout), "--run", "r1").returncode == 0
    ledger(checkout).write_text(body)
    r = run("--checkout", str(checkout), "--run", "r1")
    assert r.returncode == 1 and "Traceback" not in r.stderr
    assert json.loads(backup(checkout).read_text()) == {"v": 1}


def test_missing_ledger_or_backup_is_refused(checkout):
    r = run("--checkout", str(checkout), "--run", "r1")
    assert r.returncode == 1 and "no ledger" in r.stderr
    r = run("--checkout", str(checkout), "--run", "r1", "--restore")
    assert r.returncode == 1 and "no backup" in r.stderr


def test_a_symlinked_ledger_folder_is_refused(checkout, tmp_path):
    other = tmp_path / "elsewhere"
    other.mkdir()
    (other / "ledger.json").write_text("{}")
    (checkout / "specs" / "fleet").mkdir(parents=True)
    os.symlink(other, checkout / "specs" / "fleet" / "r1")
    r = run("--checkout", str(checkout), "--run", "r1")
    assert r.returncode == 1 and "symlink" in r.stderr


def test_not_a_git_checkout_is_refused(tmp_path):
    d = tmp_path / "plain"
    d.mkdir()
    r = run("--checkout", str(d), "--run", "r1")
    assert r.returncode == 1 and "not a git checkout" in r.stderr


@pytest.mark.parametrize("run_id", ["../x", "a/b", "", "-x"])
def test_a_bad_run_id_is_a_usage_error(checkout, run_id):
    assert run("--checkout", str(checkout), "--run", run_id).returncode == 2


def test_a_checkout_reached_through_a_symlinked_parent_works(checkout, tmp_path):
    link = tmp_path / "link"
    os.symlink(checkout.parent, link)
    write_ledger(checkout, {"v": 1})
    r = run("--checkout", str(link / "co"), "--run", "r1")
    assert r.returncode == 0, r.stderr
    assert backup(checkout).exists()


def test_the_restored_ledger_keeps_the_mode_of_its_source(checkout):
    write_ledger(checkout, {"v": 1})
    os.chmod(ledger(checkout), 0o644)
    assert run("--checkout", str(checkout), "--run", "r1").returncode == 0
    assert oct(os.stat(backup(checkout)).st_mode & 0o777) == "0o644"
    ledger(checkout).unlink()
    assert run("--checkout", str(checkout), "--run", "r1", "--restore").returncode == 0
    assert oct(os.stat(ledger(checkout)).st_mode & 0o777) == "0o644"

#!/usr/bin/env python3
"""Prepare a Pod Bay workspace checkout so the session started in it runs a fleet role unattended.

The orchestrator runs this before `podbay-dev start` and again with --restore after it stops the
session. It writes three keys into <checkout>/.claude/settings.local.json, which Claude Code reads at
session start:

  model                      the role's model (the session starts on it; no /model keystrokes)
  permissions.defaultMode    the run's permission mode (a session in another mode holds messages)
  env.PD_BOT_ROLE / env.PD_BOT_TICKET / env.PD_BOT_RUN   the role, read by the plugin's hooks

Every other key in the file is kept. The file as it was before goes to
.claude/settings.local.json.pd-bot-backup (once: a second write keeps the first backup), and
--restore puts it back, or deletes the file when there was none.

Safety:
- The file must be ignored by git in that checkout (`git check-ignore`), so the role settings are
  never committed. The script refuses otherwise.
- It refuses `bypassPermissions`: a fleet session keeps the permission checks and the hooks.
- A symlinked settings file or .claude directory is refused.

Usage:
  fleet-settings.py --checkout DIR --role worker|reviewer|tester|advisor --model ID
                    --mode auto|acceptEdits [--ticket ID] [--run ID]
  fleet-settings.py --checkout DIR --restore
Exit: 0 done, 1 refused (the reason on stderr), 2 usage error.
"""
import argparse
import json
import os
import re
import subprocess
import sys
import tempfile

SETTINGS = os.path.join(".claude", "settings.local.json")
BACKUP_SUFFIX = ".pd-bot-backup"
NO_FILE_MARK = '{"pd-bot-backup": "no file before"}\n'
ROLES = ("worker", "reviewer", "tester", "advisor")
MODES = ("auto", "acceptEdits")
MODEL_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._\[\]-]{0,79}$")
ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$")


def refuse(msg):
    print(f"fleet-settings: {msg}", file=sys.stderr)
    sys.exit(1)


def write_atomic(path, text):
    d = os.path.dirname(path)
    fd, tmp = tempfile.mkstemp(dir=d, prefix=".pd-bot-settings.")
    try:
        with os.fdopen(fd, "w") as f:
            f.write(text)
        os.chmod(tmp, 0o600)
        os.replace(tmp, path)
    except BaseException:
        if os.path.exists(tmp):
            os.unlink(tmp)
        raise


def check_paths(checkout):
    if not os.path.isdir(checkout):
        print(f"fleet-settings: --checkout is not a directory: {checkout}", file=sys.stderr)
        sys.exit(2)
    cdir = os.path.join(checkout, ".claude")
    path = os.path.join(checkout, SETTINGS)
    if os.path.islink(cdir):
        refuse(f"{cdir} is a symlink")
    for p in (path, path + BACKUP_SUFFIX):
        if os.path.islink(p):
            refuse(f"{p} is a symlink")
    return cdir, path


def ignored_by_git(checkout):
    r = subprocess.run(["git", "-C", checkout, "check-ignore", "-q", "--", SETTINGS],
                       capture_output=True)
    return r.returncode == 0


def restore(checkout):
    _, path = check_paths(checkout)
    bak = path + BACKUP_SUFFIX
    if not os.path.exists(bak):
        print("fleet-settings: nothing to restore (no backup)")
        return
    with open(bak) as f:
        before = f.read()
    if before == NO_FILE_MARK:
        if os.path.exists(path):
            os.unlink(path)
        print(f"fleet-settings: removed {path} (there was none before)")
    else:
        write_atomic(path, before)
        print(f"fleet-settings: restored {path}")
    os.unlink(bak)


def apply(a):
    cdir, path = check_paths(a.checkout)
    if not ignored_by_git(a.checkout):
        refuse(f"{SETTINGS} is not ignored by git in {a.checkout}: add it to .gitignore first, "
               "so the role settings are never committed")
    os.makedirs(cdir, exist_ok=True)
    data = {}
    before = None
    if os.path.exists(path):
        with open(path) as f:
            before = f.read()
        try:
            data = json.loads(before) if before.strip() else {}
        except ValueError:
            refuse(f"{path} is not valid JSON; fix it by hand first")
        if not isinstance(data, dict):
            refuse(f"{path} does not hold a JSON object")
    bak = path + BACKUP_SUFFIX
    if not os.path.exists(bak):
        write_atomic(bak, before if before is not None else NO_FILE_MARK)
    data["model"] = a.model
    perms = data.get("permissions")
    if perms is None:
        perms = data["permissions"] = {}
    if not isinstance(perms, dict):
        refuse(f"{path}: permissions is not an object")
    perms["defaultMode"] = a.mode
    env = data.get("env")
    if env is None:
        env = data["env"] = {}
    if not isinstance(env, dict):
        refuse(f"{path}: env is not an object")
    env["PD_BOT_ROLE"] = a.role
    for key, val in (("PD_BOT_TICKET", a.ticket), ("PD_BOT_RUN", a.run)):
        if val:
            env[key] = val
        else:
            env.pop(key, None)
    write_atomic(path, json.dumps(data, indent=2) + "\n")
    print(f"fleet-settings: {path}: model {a.model}, mode {a.mode}, role {a.role}"
          + (f", ticket {a.ticket}" if a.ticket else ""))


def main(argv):
    p = argparse.ArgumentParser(prog="fleet-settings.py")
    p.add_argument("--checkout", required=True)
    p.add_argument("--restore", action="store_true")
    p.add_argument("--role", choices=ROLES)
    p.add_argument("--model")
    p.add_argument("--mode", choices=MODES)
    p.add_argument("--ticket")
    p.add_argument("--run")
    a = p.parse_args(argv)
    if a.restore:
        if a.role or a.model or a.mode or a.ticket or a.run:
            p.error("--restore takes only --checkout")
        restore(a.checkout)
        return
    if not (a.role and a.model and a.mode):
        p.error("--role, --model and --mode are required (or --restore)")
    if not MODEL_RE.match(a.model):
        p.error("--model is not a model id")
    for name in ("ticket", "run"):
        v = getattr(a, name)
        if v is not None and not ID_RE.match(v):
            p.error(f"--{name} must be letters, digits, dot, dash or underscore")
    apply(a)


if __name__ == "__main__":
    main(sys.argv[1:])

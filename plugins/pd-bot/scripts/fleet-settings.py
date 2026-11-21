#!/usr/bin/env python3
"""Prepare a Pod Bay workspace checkout so the session started in it runs a fleet role unattended.

The orchestrator runs this before `podbay-dev start` and again with --restore after it stops the
session. It sets these keys in <checkout>/.claude/settings.local.json, which Claude Code reads at
session start:

  model                      the role's model (the session starts on it; no /model keystrokes)
  permissions.defaultMode    the run's permission mode (a session in another mode holds messages)
  env.PD_BOT_ROLE            the role; the plugin's session-role hook reads it
  env.PD_BOT_TICKET, env.PD_BOT_RUN   the ticket and the run, for the session and its commands

Every other key is kept. The values those keys had before (or that they were absent, or that the
file did not exist) are saved in the checkout's git directory (`pd-bot-fleet-settings.json` under
`git rev-parse --absolute-git-dir`), never in the work tree, so they cannot be committed. A second
apply keeps the values saved by the first. --restore puts back only those keys and keeps every
other change made meanwhile (for example a permission rule the session saved). It removes the file
when the file did not exist before and nothing else is left in it.

Safety:
- The settings file must be ignored by git in that checkout (`git check-ignore`), so the role
  settings are never committed. The script refuses otherwise.
- It refuses `bypassPermissions`: a fleet session keeps the permission checks and the hooks.
- A symlinked settings file or .claude directory is refused.
- Every check runs before anything is written.

Usage:
  fleet-settings.py --checkout DIR --role worker|reviewer|tester|advisor --model ID
                    --mode auto|acceptEdits [--ticket ID] [--run ID]
  fleet-settings.py --checkout DIR --restore
  fleet-settings.py --checkout DIR --show
--show is read-only: it prints one line per fleet key (`<key>: <JSON value>` or `<key>: absent`) and
a last line `restore state: present|none` (whether the git directory holds saved values to restore).
It writes nothing and cannot be combined with --restore or the apply options.
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
STATE_NAME = "pd-bot-fleet-settings.json"
ROLES = ("worker", "reviewer", "tester", "advisor")
MODES = ("auto", "acceptEdits")
MODEL_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._:@\[\]-]{0,99}$")
ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$")
# (path in the settings object) for every key the script sets.
KEYS = (("model",), ("permissions", "defaultMode"), ("env", "PD_BOT_ROLE"),
        ("env", "PD_BOT_TICKET"), ("env", "PD_BOT_RUN"))
ABSENT = {"absent": True}


def refuse(msg):
    print(f"fleet-settings: {msg}", file=sys.stderr)
    sys.exit(1)


def write_atomic(path, text):
    fd, tmp = tempfile.mkstemp(dir=os.path.dirname(path), prefix=".pd-bot-settings.")
    try:
        with os.fdopen(fd, "w") as f:
            f.write(text)
        os.chmod(tmp, 0o600)
        os.replace(tmp, path)
    except BaseException:
        if os.path.exists(tmp):
            os.unlink(tmp)
        raise


def git(checkout, *args):
    try:
        return subprocess.run(["git", "-C", checkout, *args], capture_output=True, text=True)
    except FileNotFoundError:
        refuse("git is not installed")


def paths(checkout):
    if not os.path.isdir(checkout):
        print(f"fleet-settings: --checkout is not a directory: {checkout}", file=sys.stderr)
        sys.exit(2)
    cdir = os.path.join(checkout, ".claude")
    path = os.path.join(checkout, SETTINGS)
    if os.path.islink(cdir):
        refuse(f"{cdir} is a symlink")
    if os.path.islink(path):
        refuse(f"{path} is a symlink")
    r = git(checkout, "rev-parse", "--absolute-git-dir")
    if r.returncode != 0 or not r.stdout.strip():
        refuse(f"{checkout} is not a git checkout")
    state = os.path.join(r.stdout.strip(), STATE_NAME)
    if os.path.islink(state):
        refuse(f"{state} is a symlink")
    return cdir, path, state


def read_settings(path):
    """The settings object, or None when the file does not exist. Refuses a bad file."""
    if not os.path.exists(path):
        return None
    with open(path) as f:
        text = f.read()
    try:
        data = json.loads(text) if text.strip() else {}
    except ValueError:
        refuse(f"{path} is not valid JSON; fix it by hand first")
    if not isinstance(data, dict):
        refuse(f"{path} does not hold a JSON object")
    for parent in ("permissions", "env"):
        if parent in data and not isinstance(data[parent], dict):
            refuse(f"{path}: {parent} is not an object")
    return data


def get(data, key):
    cur = data
    for k in key:
        if not isinstance(cur, dict) or k not in cur:
            return ABSENT
        cur = cur[k]
    return {"value": cur}


def put(data, key, saved):
    parent = data
    for k in key[:-1]:
        parent = parent.setdefault(k, {})
    if saved == ABSENT or "value" not in saved:
        parent.pop(key[-1], None)
        if len(key) > 1 and not parent:
            data.pop(key[0], None)
    else:
        parent[key[-1]] = saved["value"]


def dump(data):
    return json.dumps(data, indent=2) + "\n"


def restore(checkout):
    _, path, state = paths(checkout)
    if not os.path.exists(state):
        print("fleet-settings: nothing to restore")
        return
    with open(state) as f:
        try:
            saved = json.load(f)
        except ValueError:
            refuse(f"{state} is not valid JSON; check {path} by hand and delete it")
    data = read_settings(path) or {}
    for key in KEYS:
        put(data, key, saved.get("keys", {}).get(".".join(key), ABSENT))
    if not saved.get("file_existed", True) and not data:
        if os.path.exists(path):
            os.unlink(path)
        print(f"fleet-settings: removed {path} (there was none before)")
    else:
        write_atomic(path, dump(data))
        print(f"fleet-settings: restored the fleet keys in {path}")
    os.unlink(state)


def show(checkout):
    _, path, state = paths(checkout)
    data = read_settings(path) or {}
    for key in KEYS:
        got = get(data, key)
        print(f"{'.'.join(key)}: " + (json.dumps(got["value"]) if "value" in got else "absent"))
    print("restore state: " + ("present" if os.path.exists(state) else "none"))


def apply(a):
    cdir, path, state = paths(a.checkout)
    r = git(a.checkout, "check-ignore", "-q", "--", SETTINGS)
    if r.returncode == 1:
        refuse(f"{SETTINGS} is not ignored by git in {a.checkout}: add it to .gitignore first, "
               "so the role settings are never committed")
    if r.returncode != 0:
        refuse(f"git check-ignore failed in {a.checkout}: {r.stderr.strip()[:200]}")
    data = read_settings(path)
    existed = data is not None
    data = data or {}
    # Every check is done; now write. The first apply's saved values win over a later apply's.
    if not os.path.exists(state):
        saved = {"file_existed": existed, "keys": {".".join(k): get(data, k) for k in KEYS}}
        write_atomic(state, dump(saved))
    os.makedirs(cdir, exist_ok=True)
    new = {("model",): a.model, ("permissions", "defaultMode"): a.mode,
           ("env", "PD_BOT_ROLE"): a.role, ("env", "PD_BOT_TICKET"): a.ticket,
           ("env", "PD_BOT_RUN"): a.run}
    for key, val in new.items():
        put(data, key, {"value": val} if val else ABSENT)
    write_atomic(path, dump(data))
    print(f"fleet-settings: {path}: model {a.model}, mode {a.mode}, role {a.role}"
          + (f", ticket {a.ticket}" if a.ticket else ""))


def main(argv):
    p = argparse.ArgumentParser(prog="fleet-settings.py")
    p.add_argument("--checkout", required=True)
    p.add_argument("--restore", action="store_true")
    p.add_argument("--show", action="store_true")
    p.add_argument("--role", choices=ROLES)
    p.add_argument("--model")
    p.add_argument("--mode", choices=MODES)
    p.add_argument("--ticket")
    p.add_argument("--run")
    a = p.parse_args(argv)
    if a.show:
        if a.restore or a.role or a.model or a.mode or a.ticket or a.run:
            p.error("--show takes only --checkout")
        show(a.checkout)
        return
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

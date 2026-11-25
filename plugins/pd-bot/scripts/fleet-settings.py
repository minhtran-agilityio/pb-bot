#!/usr/bin/env python3
"""Prepare a Pod Bay workspace checkout so the session started in it runs a fleet role unattended.

The orchestrator runs this before `podbay-dev start` and again with --restore after it stops the
session. It sets these keys in <checkout>/.claude/settings.local.json, which Claude Code reads at
session start:

  model                      the role's model (the session starts on it; no /model keystrokes)
  effortLevel                the role's effort (--effort low|medium|high|xhigh), when given
  permissions.defaultMode    the run's permission mode (a session in another mode holds messages)
  env.PD_BOT_ROLE            the role; the plugin's session-role hook reads it
  env.PD_BOT_TICKET, env.PD_BOT_RUN   the ticket and the run, for the session and its commands
  disableClaudeAiConnectors, autoCompactWindow, enabledPlugins.<plugin>
                             the token trim in fleet-trim.json (next to this script): no claude.ai
                             connectors and no unused plugins, so each request carries about 20k
                             fewer tokens of tool and skill lists, and an auto-compact window of
                             500k instead of 1M. --keep-connectors skips it.

Model guard: a worker runs Sonnet. For --role worker, a model that is not a Sonnet or Haiku id or
alias (Opus, Fable, or an alias such as `best` that can resolve to Opus) is refused unless --depth L
or --escalated (a Sonnet worker already failed once on this ticket) is given.

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
                    [--depth S|M|L] [--escalated] [--keep-connectors]
                    [--effort low|medium|high|xhigh]
  fleet-settings.py --checkout DIR --restore
  fleet-settings.py --checkout DIR --show
--show is read-only: it prints one line per fleet key (`<key>: <JSON value>` or `<key>: absent`) and
a last line `restore state: present|present (invalid)|none` (whether the git directory holds saved values to restore).
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
        ("env", "PD_BOT_TICKET"), ("env", "PD_BOT_RUN"), ("effortLevel",))
ABSENT = {"absent": True}
TRIM_FILE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "fleet-trim.json")
WORKER_MODEL_RE = re.compile(r"sonnet|haiku", re.I)


def trim_values():
    """(key, value) pairs of the token trim, read from fleet-trim.json."""
    try:
        with open(TRIM_FILE) as f:
            data = json.load(f)
    except (OSError, ValueError) as e:
        refuse(f"cannot read {TRIM_FILE}: {e}")
    if not isinstance(data, dict):
        refuse(f"{TRIM_FILE} does not hold a JSON object")
    pairs = [(("disableClaudeAiConnectors",), data.get("disableClaudeAiConnectors", True))]
    pairs += [((k,), v) for k, v in sorted(data.items())
              if k not in ("disableClaudeAiConnectors", "enabledPlugins")]
    plugins = data.get("enabledPlugins", {})
    if not isinstance(plugins, dict):
        refuse(f"{TRIM_FILE}: enabledPlugins is not an object")
    pairs += [(("enabledPlugins", name), bool(on)) for name, on in sorted(plugins.items())]
    return pairs


def trim_keys():
    return tuple(k for k, _ in trim_values())


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
    for parent in ("permissions", "env", "enabledPlugins"):
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
    if not isinstance(saved, dict):
        refuse(f"{state} is not a JSON object; check {path} by hand and delete it")
    data = read_settings(path) or {}
    for key in KEYS:
        put(data, key, saved.get("keys", {}).get(".".join(key), ABSENT))
    for item in saved.get("extra", []):
        if isinstance(item, dict) and isinstance(item.get("path"), list) and item["path"]:
            put(data, tuple(item["path"]), item.get("saved", ABSENT))
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
    for key in KEYS + trim_keys():
        got = get(data, key)
        print(f"{'.'.join(key)}: " + (json.dumps(got["value"]) if "value" in got else "absent"))
    label = "none"
    if os.path.exists(state):
        label = "present"
        try:
            if not os.path.isfile(state):
                raise ValueError("not a regular file")
            with open(state) as f:
                if not isinstance(json.load(f), dict):
                    raise ValueError("not a JSON object")
        except (OSError, ValueError, RecursionError):
            label = "present (invalid)"
    print("restore state: " + label)


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
    trim = [] if a.keep_connectors else trim_values()
    # Every check is done; now write. The first apply's saved values win over a later apply's;
    # a trim key the first apply did not set is added with its value as it stands now.
    if os.path.exists(state):
        bad = f"{state} is not a valid restore state; check {path} by hand and delete it"
        try:
            with open(state) as f:
                saved = json.load(f)
        except (OSError, ValueError, RecursionError):
            refuse(bad)
        if not isinstance(saved, dict) or not isinstance(saved.get("extra", []), list):
            refuse(bad)
        for item in saved.get("extra", []):
            if not (isinstance(item, dict) and isinstance(item.get("path"), list) and item["path"]
                    and all(isinstance(k, str) for k in item["path"])):
                refuse(bad)
    else:
        saved = {"file_existed": existed, "keys": {".".join(k): get(data, k) for k in KEYS}}
    extra = saved.setdefault("extra", [])
    known = {tuple(i["path"]) for i in extra}
    for key, _ in trim:
        if key not in known:
            extra.append({"path": list(key), "saved": get(data, key)})
    if a.keep_connectors:
        # An earlier apply may have written the trim: put the owner's values back.
        for item in extra:
            put(data, tuple(item["path"]), item.get("saved", ABSENT))
    write_atomic(state, dump(saved))
    os.makedirs(cdir, exist_ok=True)
    new = {("model",): a.model, ("permissions", "defaultMode"): a.mode,
           ("env", "PD_BOT_ROLE"): a.role, ("env", "PD_BOT_TICKET"): a.ticket,
           ("env", "PD_BOT_RUN"): a.run, ("effortLevel",): a.effort}
    for key, val in new.items():
        put(data, key, {"value": val} if val else ABSENT)
    for key, val in trim:
        put(data, key, {"value": val})
    write_atomic(path, dump(data))
    print(f"fleet-settings: {path}: model {a.model}, mode {a.mode}, role {a.role}"
          + (f", effort {a.effort}" if a.effort else "")
          + (f", ticket {a.ticket}" if a.ticket else "")
          + (", token trim off" if a.keep_connectors else ", token trim on"))


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
    p.add_argument("--depth", choices=("S", "M", "L"))
    p.add_argument("--escalated", action="store_true")
    p.add_argument("--keep-connectors", action="store_true")
    p.add_argument("--effort", choices=("low", "medium", "high", "xhigh"))
    a = p.parse_args(argv)
    apply_opts = (a.role, a.model, a.mode, a.ticket, a.run, a.depth, a.effort,
                  a.escalated or None, a.keep_connectors or None)
    if a.show:
        if a.restore or any(v is not None for v in apply_opts):
            p.error("--show takes only --checkout")
        show(a.checkout)
        return
    if a.restore:
        if any(v is not None for v in apply_opts):
            p.error("--restore takes only --checkout")
        restore(a.checkout)
        return
    if not (a.role and a.model and a.mode):
        p.error("--role, --model and --mode are required (or --restore or --show)")
    if not MODEL_RE.match(a.model):
        p.error("--model is not a model id")
    for name in ("ticket", "run"):
        v = getattr(a, name)
        if v is not None and not ID_RE.match(v):
            p.error(f"--{name} must be letters, digits, dot, dash or underscore")
    if (a.role == "worker" and not WORKER_MODEL_RE.search(a.model)
            and not (a.depth == "L" or a.escalated)):
        refuse(f"a worker runs Sonnet; {a.model} needs --depth L, or --escalated after a Sonnet "
               "worker failed once on this ticket")
    apply(a)


if __name__ == "__main__":
    main(sys.argv[1:])

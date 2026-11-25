#!/usr/bin/env python3
"""Keep a second copy of a fleet run's ledger in the checkout's git directory.

The ledger `specs/fleet/<run>/ledger.json` is the orchestrator's memory, and `specs/` is gitignored:
`git clean -fdx` or a lost `specs/` folder would take it away. After each ledger write the
orchestrator runs this script, which copies the ledger to
`<git dir>/pd-bot/fleet/<run>/ledger.json` (`git rev-parse --absolute-git-dir`). That copy is never
committed and survives `git clean`; it is lost with the checkout itself.

Usage:
  ledger-backup.py --checkout DIR --run RUN             copy the ledger to the git directory
  ledger-backup.py --checkout DIR --run RUN --restore   copy it back when the ledger is missing
Exit: 0 done, 1 refused (the reason on stderr), 2 usage error.
"""
import argparse
import json
import os
import re
import subprocess
import sys
import tempfile

RUN_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$")


def refuse(msg):
    print(f"ledger-backup: {msg}", file=sys.stderr)
    sys.exit(1)


def git_dir(checkout):
    try:
        r = subprocess.run(["git", "-C", checkout, "rev-parse", "--absolute-git-dir"],
                           capture_output=True, text=True)
    except FileNotFoundError:
        refuse("git is not installed")
    if r.returncode != 0 or not r.stdout.strip():
        refuse(f"{checkout} is not a git checkout")
    return r.stdout.strip()


def read_ledger(path):
    try:
        with open(path) as f:
            data = json.load(f)
    except (OSError, ValueError, RecursionError) as e:
        refuse(f"cannot read {path} as JSON: {e}")
    if not isinstance(data, dict):
        refuse(f"{path} does not hold a JSON object")
    return data


def no_links(*paths):
    for p in paths:
        cur = p
        while cur and cur != os.path.dirname(cur):
            if os.path.islink(cur):
                refuse(f"{cur} is a symlink")
            cur = os.path.dirname(cur)


def copy(src, dst):
    data = read_ledger(src)
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=os.path.dirname(dst), prefix=".ledger.")
    try:
        with os.fdopen(fd, "w") as f:
            json.dump(data, f, indent=2)
            f.write("\n")
        os.replace(tmp, dst)
    except BaseException:
        if os.path.exists(tmp):
            os.unlink(tmp)
        raise


def main(argv):
    p = argparse.ArgumentParser(prog="ledger-backup.py")
    p.add_argument("--checkout", required=True)
    p.add_argument("--run", required=True)
    p.add_argument("--restore", action="store_true")
    a = p.parse_args(argv)
    if not RUN_RE.match(a.run):
        p.error("--run must be letters, digits, dot, dash or underscore")
    if not os.path.isdir(a.checkout):
        p.error(f"--checkout is not a directory: {a.checkout}")
    checkout = os.path.abspath(a.checkout)
    ledger = os.path.join(checkout, "specs", "fleet", a.run, "ledger.json")
    backup = os.path.join(git_dir(checkout), "pd-bot", "fleet", a.run, "ledger.json")
    no_links(ledger, backup)
    if a.restore:
        if os.path.exists(ledger):
            refuse(f"{ledger} exists; not overwriting it with the backup")
        if not os.path.exists(backup):
            refuse(f"no backup for run {a.run} at {backup}")
        copy(backup, ledger)
        print(f"ledger-backup: restored {ledger} from {backup}")
        return
    if not os.path.exists(ledger):
        refuse(f"no ledger at {ledger}")
    copy(ledger, backup)
    print(f"ledger-backup: saved {backup}")


if __name__ == "__main__":
    main(sys.argv[1:])

#!/usr/bin/env python3
"""Fail when a skill's `allowed-tools` pre-allows a wide rule: one that can run any command,
push to any ref, read or write any file, or post any local file (issue #123).

Why: when a turn starts with a typed slash command, Claude Code applies the skill's
`allowed-tools` without a prompt, even under `--permission-mode dontAsk` (probed with haiku,
Claude Code 2.1.330). A `*` in a Bash rule matches any further arguments, so:
  - `Bash(git -C *)` allowed `git -C <dir> -c core.fsmonitor='<cmd>' status`, which ran <cmd>;
  - `Bash(git log *)` allowed `git log --output=<file>`, which wrote a file of its choice;
  - a bare `Read`, `Write` or `Edit` read and wrote files outside the repo.
Prompt injection in a file or an issue that a skill reads could use any of these.

Verifies:        every entry of every plugins/pd-bot/skills/*/SKILL.md `allowed-tools` against the
                 WIDE rules below. A wide entry passes only when the baseline file lists it for
                 that skill; a baseline entry that the skill no longer has fails too, so the
                 baseline only shrinks. Skills in NEVER_WIDE may have no baseline entry at all.
Does not verify: that a narrow rule is narrow enough in every Claude Code version, or that a skill
                 still works with fewer grants (check_skill_tool_conformance.py covers the body's
                 commands).

Stdlib only. Exit 0 = clean; 1 = wide rules found; 2 = could not run.
Run: python3 scripts/check_allowed_tools_wide.py [--root DIR] [--baseline FILE] [--list]
"""

import argparse
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
DEFAULT_ROOT = HERE.parent / "plugins" / "pd-bot"
DEFAULT_BASELINE = HERE / "allowed-tools-wide-baseline.json"

# Skills that must never pre-allow a wide rule, baseline or not (issue #123).
NEVER_WIDE = {"pd-shape"}

# Commands that run code given as an argument, or run another command.
INTERPRETERS = {
    "python", "python3", "bash", "sh", "zsh", "node", "perl", "ruby", "env", "xargs",
    "eval", "exec", "sudo", "npx", "uv", "deno",
}

# gh verbs that only read. Any other gh verb with a `*` can post a local file
# (`--body-file ~/.ssh/id_rsa`) or change the remote.
GH_READ_VERBS = {"view", "list", "status", "diff", "checks"}

# File tools: bare, or with a path pattern that covers the whole disk, means any file.
FILE_TOOLS = {"Read", "Write", "Edit", "MultiEdit", "NotebookEdit", "Grep", "Glob"}
WHOLE_DISK = {"*", "**", "/**", "//**", "~/**", "~/*", "/*", "//*"}


def frontmatter(text: str):
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return None
    for i in range(1, len(lines)):
        if lines[i].strip() == "---":
            return "\n".join(lines[1:i])
    return None


def entries(block: str) -> list[str]:
    """The comma-separated `allowed-tools` entries; a comma inside (...) does not split."""
    match = re.search(r"^allowed-tools:(.*)$", block, re.MULTILINE)
    if match is None:
        return []
    out, depth, cur = [], 0, ""
    for ch in match.group(1).strip():
        depth += ch == "("
        depth -= ch == ")"
        if ch == "," and depth == 0:
            out.append(cur.strip())
            cur = ""
        else:
            cur += ch
    if cur.strip():
        out.append(cur.strip())
    return [e for e in out if e]


def why_wide(entry: str):
    """Return the reason an entry is wide, or None when it is narrow."""
    if entry == "Bash":
        return "bare Bash pre-allows every command"
    tool = re.match(r"^(\w+)(?:\((.*)\))?$", entry, re.DOTALL)
    if tool is None:
        return None
    name, arg = tool.group(1), tool.group(2)
    if name in FILE_TOOLS:
        if arg is None:
            return f"bare {name} pre-allows every file on the disk"
        if arg.strip() in WHOLE_DISK:
            return f"{name}({arg}) covers every file on the disk"
        return None
    if name != "Bash" or arg is None:
        return None

    rule = arg.strip()
    words = rule.split()
    if not words or "*" not in rule:
        return None
    if words[0] == "*":
        return "a leading * matches every command"
    # A `*` joined to a word (`run-tests.sh*`, `docs/adr*`) matches other files and extra words.
    for word in words:
        if word != "*" and word.endswith("*") and not word.endswith("=*"):
            return f"`{word}`: a * joined to a word matches other paths and extra arguments"
    head = words[0]
    if head == "git":
        return "git with a *: options such as -c core.fsmonitor=<cmd>, --output=<file> or --upload-pack=<cmd> run commands or write files"
    if head in INTERPRETERS:
        nxt = words[1] if len(words) > 1 else "*"
        if "*" in nxt or nxt.startswith("-"):
            return f"{head} followed by * or an option runs any code"
        return None
    if head == "gh":
        if len(words) < 3 or "*" in words[1] or "*" in words[2]:
            return "gh with a * in place of the command group or verb"
        if words[1] == "api":
            return "gh api with a * can send any request"
        if words[2] not in GH_READ_VERBS:
            return f"gh {words[1]} {words[2]} with a * can post a local file (--body-file) or change the remote"
    return None


def scan(root: Path) -> dict[str, list[tuple[str, str]]]:
    """Map each skill to its wide (entry, reason) pairs."""
    found: dict[str, list[tuple[str, str]]] = {}
    for skill_md in sorted((root / "skills").glob("*/SKILL.md")):
        block = frontmatter(skill_md.read_text())
        if block is None:
            continue
        wide = [(e, r) for e in entries(block) if (r := why_wide(e))]
        if wide:
            found[skill_md.parent.name] = wide
    return found


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=DEFAULT_ROOT)
    parser.add_argument("--baseline", type=Path, default=DEFAULT_BASELINE)
    parser.add_argument("--list", action="store_true", help="print every wide entry, baseline or not")
    args = parser.parse_args()

    if not list((args.root / "skills").glob("*/SKILL.md")):
        print(f"  ✗ allowed-tools width: no skills/*/SKILL.md under {args.root}", file=sys.stderr)
        return 2
    try:
        baseline = json.loads(args.baseline.read_text()) if args.baseline.exists() else {}
    except json.JSONDecodeError as err:
        print(f"  ✗ allowed-tools width: bad baseline {args.baseline}: {err}", file=sys.stderr)
        return 2
    baseline = {k: v for k, v in baseline.items() if not k.startswith("_")}

    found = scan(args.root)
    if args.list:
        for skill, wide in found.items():
            for entry, reason in wide:
                print(f"{skill}\t{entry}\t{reason}")

    problems = []
    for skill in sorted(NEVER_WIDE & set(baseline)):
        problems.append(f"{skill}: is in NEVER_WIDE and may have no baseline entry")
    for skill, wide in found.items():
        allowed = set(baseline.get(skill, [])) if skill not in NEVER_WIDE else set()
        for entry, reason in wide:
            if entry not in allowed:
                problems.append(f"{skill}: `{entry}` is wide ({reason})")
    for skill, listed in baseline.items():
        have = {e for e, _ in found.get(skill, [])}
        for entry in listed:
            if entry not in have:
                problems.append(f"{skill}: baseline entry `{entry}` is gone; remove it from {args.baseline.name}")

    if problems:
        print("  ✗ allowed-tools width:", file=sys.stderr)
        for line in problems:
            print(f"      {line}", file=sys.stderr)
        print(
            "      Narrow the rule (a fixed command with no *, a path such as Read(./**) or Edit(./specs/**)),\n"
            "      or drop it and let the owner approve each call. Never add a skill to the baseline to pass.",
            file=sys.stderr,
        )
        return 1
    known = sum(len(v) for v in baseline.values())
    print(f"  ✓ allowed-tools width: no new wide rule ({known} known wide entries in the baseline, none in {', '.join(sorted(NEVER_WIDE))})")
    return 0


if __name__ == "__main__":
    sys.exit(main())

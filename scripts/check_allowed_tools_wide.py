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
                 commands). Known limit: it judges only Bash and the file tools. WebFetch,
                 WebSearch, Agent, Task, Skill, SendMessage and MCP tools (`mcp__...`) pass
                 unjudged, although WebFetch with a wide Read can send a file out (a follow-up
                 ticket covers them).

Shrink check: each baseline entry must also be in the baseline file at --base-ref (default
origin/main). It fails closed: when git or the ref is missing, the run fails. A ref that has no
baseline file yet (the change that adds it) passes with a note. `--base-ref ""` skips the check
on purpose (tests on temp trees).

Stdlib only. Exit 0 = clean; 1 = wide rules found, or the shrink check cannot run; 2 = could not run.
Run: python3 scripts/check_allowed_tools_wide.py [--root DIR] [--baseline FILE] [--base-ref REF] [--list]
"""

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
DEFAULT_ROOT = HERE.parent / "plugins" / "pd-bot"
DEFAULT_BASELINE = HERE / "allowed-tools-wide-baseline.json"

# Skills that must never pre-allow a wide rule, baseline or not (issue #123).
NEVER_WIDE = {"pd-shape"}

# Fail closed: a Bash rule with a `*` is wide unless it has one of these reviewed shapes.
# 1. A command that only reads and has no flag that runs code or writes a file.
SAFE_STAR_COMMANDS = {"ls", "test"}
# 2. `gh <group> <verb> *` with a verb that only reads. Any other gh verb with a `*` can post a
#    local file (`--body-file ~/.ssh/id_rsa`) or change the remote; `gh auth` can print the token.
GH_READ_VERBS = {"view", "list", "diff", "checks"}
# 3. An interpreter, then a fixed script path, then ` *`: the script parses the arguments. The
#    interpreter itself sees no option (probe: `Bash(python3 /abs/x.py *)` did not match
#    `python3 -c '<code>' /abs/x.py`).
INTERPRETERS = {"python", "python3", "bash", "sh", "node"}

# File tools: a bare name means every file on the disk. A path rule passes only inside the repo
# (`./...`, no `..`), and a write rule must not cover the repo root, `.git/` or `.claude/`
# (git config and settings run commands). READ_OUTSIDE lists reviewed reads outside the repo.
FILE_TOOLS = {"Read", "Write", "Edit", "MultiEdit", "NotebookEdit", "Grep", "Glob"}
WRITE_TOOLS = {"Write", "Edit", "MultiEdit", "NotebookEdit"}
# Probed: it reads the installed pd-bot's files and denies another plugin's files.
READ_OUTSIDE = {"Read(~/.claude/plugins/cache/*/pd-bot/**)"}
# Repo paths where a written file runs as code or config (CI, hooks, harness scripts, settings).
# A write rule that can reach any of them is wide.
PROTECTED_WRITE = (".git", ".claude", ".github", "scripts", "hooks",
                   "plugins/pd-bot/hooks", "plugins/pd-bot/scripts")


def frontmatter(text: str):
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return None
    for i in range(1, len(lines)):
        if lines[i].strip() == "---":
            return "\n".join(lines[1:i])
    return None


def entries(block: str) -> list[str]:
    """The comma-separated `allowed-tools` entries; a comma inside (...) does not split.

    A value this parser cannot read safely (quoted, a [list], continued on the next line, or
    given twice) comes back as one entry that why_wide() reports, so it fails closed.
    """
    matches = list(re.finditer(r"^allowed-tools:(.*)$", block, re.MULTILINE))
    if not matches:
        return []
    raw = matches[0].group(1).strip()
    rest = block[matches[0].end():].split("\n")
    continued = len(rest) > 1 and rest[1][:1] in (" ", "\t")
    if len(matches) > 1 or continued or raw[:1] in ("'", '"', "[", "|", ">") or not raw:
        return [f"<unparsed allowed-tools: {raw}>"]
    out, depth, cur = [], 0, ""
    for ch in raw:
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
    if entry.startswith("<unparsed"):
        return "the allowed-tools value is quoted, a list, continued or repeated; write it on one plain line"
    if entry == "Bash":
        return "bare Bash pre-allows every command"
    tool = re.match(r"^(\w+)(?:\((.*)\))?$", entry, re.DOTALL)
    if tool is None:
        return "entry does not parse as Tool or Tool(rule)"
    name, arg = tool.group(1), tool.group(2)
    if name in FILE_TOOLS:
        return file_rule_wide(entry, name, arg)
    if name != "Bash" or arg is None:
        return None
    return bash_rule_wide(arg.strip())


def file_rule_wide(entry: str, name: str, arg):
    if arg is None:
        return f"bare {name} pre-allows every file on the disk"
    if entry in READ_OUTSIDE:
        return None
    path = arg.strip()
    if not path.startswith("./") or ".." in path.split("/"):
        return f"{entry} reaches outside the repo; use a ./ path"
    if name in WRITE_TOOLS:
        # The fixed part of the path: the segments before the first one with a wildcard.
        fixed = []
        for seg in path[2:].split("/"):
            if any(c in seg for c in "*?[{"):
                break
            fixed.append(seg)
        if len(fixed) == len(path[2:].split("/")):
            fixed = fixed[:-1]  # a plain file path: judge its folder
        prefix = "/".join(fixed)
        for protected in PROTECTED_WRITE:
            if prefix == "" or protected == prefix or protected.startswith(prefix + "/") \
                    or prefix.startswith(protected + "/"):
                return (f"{entry} can write the repo root or {protected}/, where a file runs as "
                        "code or config")
    return None


def bash_rule_wide(rule: str):
    words = rule.split()
    if not words or "*" not in rule:
        return None
    head = words[0]
    # A `*` joined to a word (`run-tests.sh*`, `docs/adr*`) matches other files and extra words.
    for word in words:
        if word != "*" and word.endswith("*") and not word.endswith("=*"):
            return f"`{word}`: a * joined to a word matches other paths and extra arguments"
    tail_only = words[-1] == "*" and "*" not in " ".join(words[:-1])
    if head in SAFE_STAR_COMMANDS and tail_only:
        return None
    if head == "gh" and tail_only and len(words) == 4 and words[1] not in ("auth", "api") \
            and words[2] in GH_READ_VERBS:
        return None
    if head in INTERPRETERS and tail_only and len(words) == 3 and "/" in words[1] \
            and not words[1].startswith("-"):
        return None
    if head == "git":
        return "git with a *: options such as -c core.fsmonitor=<cmd>, --output=<file> or --upload-pack=<cmd> run commands or write files"
    if head == "gh":
        return "gh with a * other than `gh <group> view|list|diff|checks *` can post a local file (--body-file) or change the remote"
    if head in INTERPRETERS:
        return f"{head} with a * other than `{head} <script path> *` runs any code"
    return f"{head} with a * is not a reviewed shape (ls, test, gh read verbs, interpreter plus script path)"


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


def base_baseline(ref: str, path: Path):
    """Return (state, data) for the baseline file at `ref`.

    state is "ok" (data is the file), "absent" (the ref has no baseline file yet), or "error"
    (data is the reason: no git, not a repository, the ref does not resolve, bad JSON).
    """
    def git(*argv):
        return subprocess.run(["git", *argv], capture_output=True, text=True, timeout=10)

    try:
        top = git("-C", str(path.resolve().parent), "rev-parse", "--show-toplevel")
        if top.returncode != 0:
            return "error", f"{path.parent} is not in a git repository"
        root = top.stdout.strip()
        if git("-C", root, "rev-parse", "--verify", "--quiet", f"{ref}^{{commit}}").returncode != 0:
            return "error", f"the base ref {ref} does not resolve (fetch it, or pass --base-ref)"
        rel = path.resolve().relative_to(Path(root).resolve()).as_posix()
        if git("-C", root, "cat-file", "-e", f"{ref}:{rel}").returncode != 0:
            return "absent", None
        shown = git("-C", root, "show", f"{ref}:{rel}")
    except (OSError, ValueError, subprocess.TimeoutExpired) as err:
        return "error", f"git failed: {err}"
    if shown.returncode != 0:
        return "error", f"git show {ref}:{rel} failed"
    try:
        data = json.loads(shown.stdout)
    except json.JSONDecodeError:
        return "error", f"the baseline at {ref} is not valid JSON"
    return "ok", {k: v for k, v in data.items() if not k.startswith("_")}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=DEFAULT_ROOT)
    parser.add_argument("--baseline", type=Path, default=DEFAULT_BASELINE)
    parser.add_argument("--list", action="store_true", help="print every wide entry, baseline or not")
    parser.add_argument("--base-ref", default="origin/main",
                        help='a baseline entry must also be in this ref\'s baseline; fails when the ref is missing; "" skips')
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
    note = ""
    if args.base_ref == "":
        note = "; shrink check skipped (--base-ref \"\")"
    else:
        state, base = base_baseline(args.base_ref, args.baseline)
        if state == "error":
            problems.append(f"shrink check cannot run: {base}")
        elif state == "absent":
            note = f"; {args.base_ref} has no baseline yet, so this change adds it"
    if args.base_ref and state == "ok":
        for skill, listed in baseline.items():
            for entry in listed:
                if entry not in base.get(skill, []):
                    problems.append(f"{skill}: baseline entry `{entry}` is not in {args.base_ref}; the baseline only shrinks")
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
    print(f"  ✓ allowed-tools width: no new wide rule ({known} known wide entries in the baseline, none in {', '.join(sorted(NEVER_WIDE))}){note}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

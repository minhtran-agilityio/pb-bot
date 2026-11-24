#!/usr/bin/env python3
"""Check a project wiki: every quote is in its source, every source is tracked by git.

The wiki is a folder of markdown pages (default `docs/wiki`) that agents read on demand. This
script is the deterministic gate for it. It uses no model and no network. Stdlib only.

Page format (see `skills/pd-wiki/templates/page.md`):

    ---
    title: Hard rules
    read_when: before a merge, a push or a commit
    sources:
      - plugins/pd-bot/rules/behavior.md
    ---

    > source: plugins/pd-bot/rules/behavior.md
    > Never merge into main or the base branch on your own choice.

A quote block is a blockquote whose first line is `> source: <path>`. The other lines, joined,
must be in that file word for word (runs of whitespace count as one space).

Verifies:
  index     `index.md` exists; every page is linked from it; it is within --max-index bytes
  page      frontmatter has title, read_when and sources; at least one quote block;
            the page is within --max-page bytes
  tracked   every `sources:` path, every quote source and every relative link target is a file
            that git tracks (so it exists in a fresh clone on any machine). A link or a source
            in a gitignored folder such as `specs/` fails, also when the file exists locally
  adr       every tracked `*.md` file in --adr-dir is a source of some page, so a new decision
            record fails the check until the wiki names it
  quote     the quote text is in its source file (the working tree, or --ref <git ref>)
  golden    every entry of `golden.json` names a page that holds each `answer_contains` string
  stale     (report) pages with a source that a later commit changed, after the page's own last
            commit; --strict makes it fail. No commit id is stored, so a squash merge is safe

Does not verify: that a page is complete, that prose between the quotes is true, or that a model
answers a golden question correctly. A quote proves that the source says it, not that the page
explains it well.

Exit 0 = clean. Exit 1 = at least one FAIL line (or a STALE line with --strict). Exit 2 = usage.
Run: python3 wiki_check.py [--root DIR] [--dir docs/wiki] [--adr-dir docs/adr] [--ref REF] [--strict]
                           [--max-index 4096] [--max-page 12288]
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

NOT_PAGES = ("index.md", "log.md")
REQUIRED = ("title", "read_when", "sources")
LINK = re.compile(r"\]\(([^)\s]+)\)")


def git(root: Path, *args: str) -> tuple[int, str]:
    try:
        out = subprocess.run(["git", "-C", str(root), *args], capture_output=True, text=True)
    except OSError:
        return 127, ""
    return out.returncode, out.stdout


def norm(text: str) -> str:
    return " ".join(text.split())


def split_frontmatter(text: str) -> tuple[dict[str, object], str]:
    """Return (fields, body). Scalars are strings; `sources:` is a list of strings."""
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return {}, text
    for end in range(1, len(lines)):
        if lines[end].strip() == "---":
            break
    else:
        return {}, text
    fields: dict[str, object] = {}
    key = ""
    for line in lines[1:end]:
        item = re.match(r"^\s+-\s+(.*)$", line)
        if item and key:
            fields.setdefault(key, [])
            if isinstance(fields[key], list):
                fields[key].append(item.group(1).strip().strip("\"'"))  # type: ignore[union-attr]
            continue
        pair = re.match(r"^([A-Za-z_]+):\s*(.*)$", line)
        if pair:
            key = pair.group(1)
            value = pair.group(2).strip().strip("\"'")
            fields[key] = value if value else []
    return fields, "\n".join(lines[end + 1 :])


def quote_blocks(body: str) -> list[tuple[str, str]]:
    """Return (source path, quote text) for every `> source:` blockquote outside fenced code."""
    blocks: list[tuple[str, str]] = []
    fence = False
    current: list[str] | None = None
    source = ""
    for line in body.splitlines() + [""]:
        if re.match(r"^\s*(```|~~~)", line):
            fence = not fence
        quoted = (not fence) and line.startswith(">")
        if quoted:
            content = line[1:].strip()
            if current is None:
                head = re.match(r"^source:\s*`?([^`\s]+)`?\s*$", content)
                if head:
                    source, current = head.group(1), []
            else:
                current.append(content)
        else:
            if current is not None:
                blocks.append((source, norm(" ".join(current))))
            current = None
    return blocks


def links(body: str) -> list[str]:
    found: list[str] = []
    fence = False
    for line in body.splitlines():
        if re.match(r"^\s*(```|~~~)", line):
            fence = not fence
            continue
        if not fence:
            found.extend(LINK.findall(line))
    return found


class Check:
    def __init__(self, root: Path, wiki: str, ref: str | None, adr_dir: str = "docs/adr"):
        self.root, self.wiki, self.ref = root, wiki.strip("/"), ref
        self.adr_dir = adr_dir.strip("/")
        self.all_sources: set[str] = set()
        self.fails: list[str] = []
        self.stale: list[str] = []
        self.counts = {"pages": 0, "quotes": 0, "golden": 0, "adrs": 0}
        code, out = git(root, "ls-files", "-z")
        self.has_git = code == 0
        self.tracked = set(out.split("\0")) if self.has_git else set()

    def fail(self, msg: str) -> None:
        self.fails.append(msg)

    def is_tracked(self, path: str) -> bool:
        return path in self.tracked

    def why(self, path: str) -> str:
        """Say why a path is not usable: git ignores it (local only), or it is just not tracked."""
        if git(self.root, "check-ignore", "-q", "--", path)[0] == 0:
            return "which git ignores (local only: another machine does not have it)"
        return "which git does not track"

    def read_source(self, path: str) -> str | None:
        if self.ref:
            code, out = git(self.root, "show", f"{self.ref}:{path}")
            return out if code == 0 else None
        try:
            return (self.root / path).read_text(encoding="utf-8", errors="replace")
        except OSError:
            return None

    def resolve(self, page: str, target: str) -> str | None:
        target = target.split("#")[0].split("?")[0]
        if not target or re.match(r"^[A-Za-z][A-Za-z0-9+.-]*:", target):
            return None
        base = self.root if target.startswith("/") else (self.root / page).parent
        full = (base / target.lstrip("/")).resolve()
        try:
            return full.relative_to(self.root.resolve()).as_posix()
        except ValueError:
            return "(outside the repository) " + target

    def run(self, max_index: int, max_page: int) -> None:
        wiki_dir = self.root / self.wiki
        if not self.has_git:
            self.fail("tracked: not a git repository, so no path can be checked")
            return
        index = wiki_dir / "index.md"
        if not index.is_file():
            self.fail(f"index: {self.wiki}/index.md is missing")
            return
        index_rel = f"{self.wiki}/index.md"
        index_text = index.read_text(encoding="utf-8")
        if len(index_text.encode()) > max_index:
            self.fail(f"index: {index_rel} is {len(index_text.encode())} bytes, limit {max_index}")
        linked = {self.resolve(index_rel, t) for t in links(index_text)}
        for target in sorted(t for t in linked if t):
            if not self.is_tracked(target):
                self.fail(f"tracked: {index_rel} links to {target}, {self.why(target)}")

        pages = sorted(p for p in wiki_dir.glob("*.md") if p.name not in NOT_PAGES)
        for page in pages:
            rel = f"{self.wiki}/{page.name}"
            self.counts["pages"] += 1
            text = page.read_text(encoding="utf-8")
            if not self.is_tracked(rel):
                self.fail(f"tracked: {rel} is not tracked by git (ignored or not added)")
            if rel not in linked:
                self.fail(f"index: {rel} is not linked from index.md")
            if len(text.encode()) > max_page:
                self.fail(f"page: {rel} is {len(text.encode())} bytes, limit {max_page}")
            fields, body = split_frontmatter(text)
            for key in REQUIRED:
                if not fields.get(key):
                    self.fail(f"page: {rel} has no `{key}` in its frontmatter")
            sources = fields.get("sources") if isinstance(fields.get("sources"), list) else []
            self.all_sources.update(sources)  # type: ignore[arg-type]
            for src in sources:  # type: ignore[union-attr]
                if not self.is_tracked(src):
                    self.fail(f"tracked: {rel} names the source {src}, {self.why(src)}")
            for target in links(body):
                resolved = self.resolve(rel, target)
                if resolved and not self.is_tracked(resolved):
                    self.fail(f"tracked: {rel} links to {resolved}, {self.why(resolved)}")
            blocks = quote_blocks(body)
            if not blocks:
                self.fail(f"page: {rel} has no quote block (`> source: <path>`)")
            for src, quote in blocks:
                self.counts["quotes"] += 1
                short = quote[:60] + ("..." if len(quote) > 60 else "")
                if not self.is_tracked(src):
                    self.fail(f"tracked: {rel} quotes {src}, {self.why(src)}")
                    continue
                if src not in sources:  # type: ignore[operator]
                    self.fail(f"page: {rel} quotes {src} but does not list it under `sources:`")
                if not quote:
                    self.fail(f"quote: {rel} has an empty quote from {src}")
                    continue
                content = self.read_source(src)
                if content is None:
                    where = f"at {self.ref}" if self.ref else "in the working tree"
                    self.fail(f"quote: {rel}: cannot read {src} {where}")
                elif quote not in norm(content):
                    self.fail(f'quote: {rel}: not found in {src}: "{short}"')
            self.check_stale(rel, sources)  # type: ignore[arg-type]
        self.check_adrs()
        self.check_golden(wiki_dir)

    def check_adrs(self) -> None:
        """Every tracked decision record must be a source of some page, so a new ADR cannot be missed."""
        if not self.adr_dir:
            return
        prefix = self.adr_dir + "/"
        for path in sorted(t for t in self.tracked if t.startswith(prefix) and t.endswith(".md")):
            self.counts["adrs"] += 1
            if path not in self.all_sources:
                self.fail(f"adr: {path} is not a source of any wiki page (add it to the decisions page)")

    def check_stale(self, rel: str, sources: list[str]) -> None:
        """A page is stale when a commit after the page's own last commit changed one of its sources."""
        if not sources:
            return
        code, out = git(self.root, "log", "-1", "--format=%H", "--", rel)
        last = out.strip()
        if code != 0 or not last:
            return  # the page is not committed yet
        code, out = git(self.root, "log", "--format=", "--name-only", f"{last}..HEAD", "--", *sources)
        changed = sorted({line for line in out.splitlines() if line})
        if code == 0 and changed:
            self.stale.append(f"{rel}: changed after the page's last commit {last[:7]}: {', '.join(changed)}")

    def check_golden(self, wiki_dir: Path) -> None:
        golden = wiki_dir / "golden.json"
        if not golden.is_file():
            return
        try:
            entries = json.loads(golden.read_text(encoding="utf-8"))
            assert isinstance(entries, list)
        except (ValueError, AssertionError):
            self.fail(f"golden: {self.wiki}/golden.json is not a JSON array")
            return
        for n, entry in enumerate(entries, 1):
            self.counts["golden"] += 1
            if not isinstance(entry, dict) or not entry.get("q") or not entry.get("page"):
                self.fail(f"golden: entry {n} needs `q`, `page` and `answer_contains`")
                continue
            page = wiki_dir / str(entry["page"])
            wanted = entry.get("answer_contains") or []
            if not page.is_file():
                self.fail(f"golden: entry {n} names the page {entry['page']}, which is missing")
                continue
            if not wanted:
                self.fail(f"golden: entry {n} has no `answer_contains`")
            raw = page.read_text(encoding="utf-8")
            text = norm(re.sub(r"(?m)^>\s?", "", raw)).lower()
            for piece in wanted:
                if norm(str(piece)).lower() not in text:
                    self.fail(f'golden: entry {n} ("{entry["q"]}"): {entry["page"]} lacks "{piece}"')


def main() -> int:
    parser = argparse.ArgumentParser(description="Check a project wiki against its sources.")
    parser.add_argument("--root", default=".", help="repository root (default: the current directory)")
    parser.add_argument("--dir", default="docs/wiki", help="wiki folder, relative to the root")
    parser.add_argument("--ref", default=None, help="read quote sources at this git ref, not the tree")
    parser.add_argument("--adr-dir", default="docs/adr",
                        help="decision records that the wiki must cover; an empty value skips the check")
    parser.add_argument("--strict", action="store_true", help="a stale page fails the check")
    parser.add_argument("--max-index", type=int, default=4096)
    parser.add_argument("--max-page", type=int, default=12288)
    args = parser.parse_args()

    root = Path(args.root)
    if not root.is_dir():
        print(f"wiki-check: no such directory: {root}", file=sys.stderr)
        return 2
    check = Check(root, args.dir, args.ref, args.adr_dir)
    check.run(args.max_index, args.max_page)
    for line in check.fails:
        print(f"FAIL  {line}")
    for line in check.stale:
        print(f"STALE {line}")
    c = check.counts
    where = f"sources at {args.ref}" if args.ref else "sources in the working tree"
    print(f"wiki-check: {c['pages']} pages, {c['quotes']} quotes, {c['adrs']} ADRs covered, "
          f"{c['golden']} golden questions, "
          f"{len(check.fails)} failed, {len(check.stale)} stale ({where})")
    if check.fails or (args.strict and check.stale):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())

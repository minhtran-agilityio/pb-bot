#!/usr/bin/env python3
"""Fail when a tracked .py file uses syntax that Python 3.9 cannot run.

macOS ships /usr/bin/python3 as 3.9, so the harness scripts must import there. Two things are
checked, from the source alone (this check runs on any Python 3.8 or newer):
  1. Every file parses with the 3.9 grammar (`match`, parenthesised `with`, `except*`).
  2. No `X | Y` union in an annotation unless the file has
     `from __future__ import annotations` (without it, 3.9 raises TypeError at import).
     A `|` whose operand is a type (None, a builtin type name, a generic alias such as `list[int]`,
     or a class-like name) is flagged anywhere else, because it is the same crash at run time.

Usage: check_py39_compat.py [root]   (default: the repository root; lists files with git)
Exit 0 clean, 1 on a finding.
"""
from __future__ import annotations

import ast
import subprocess
import sys
from pathlib import Path


def has_future_annotations(tree: ast.Module) -> bool:
    return any(
        isinstance(n, ast.ImportFrom) and n.module == "__future__"
        and any(a.name == "annotations" for a in n.names)
        for n in tree.body
    )


def annotation_nodes(tree: ast.AST):
    for n in ast.walk(tree):
        if isinstance(n, ast.arg) and n.annotation is not None:
            yield n.annotation
        elif isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef)) and n.returns is not None:
            yield n.returns
        elif isinstance(n, ast.AnnAssign):
            yield n.annotation


BUILTIN_TYPES = {"int", "str", "float", "bool", "bytes", "bytearray", "complex", "list", "dict",
                 "set", "frozenset", "tuple", "type", "object", "range", "slice"}


def is_type_like(n: ast.AST) -> bool:
    """True for an operand that is a type, so `a | b` builds a union that 3.9 cannot: None, a
    generic alias (`list[int]`), a builtin type name, or a Name/Attribute spelled like a class."""
    if isinstance(n, ast.Constant):
        return n.value is None
    if isinstance(n, ast.Subscript):
        return True
    if isinstance(n, (ast.Name, ast.Attribute)):
        ident = n.id if isinstance(n, ast.Name) else n.attr
        return ident in BUILTIN_TYPES or (ident[:1].isupper() and not ident.isupper())
    return False


def check_source(text: str, name: str = "<src>") -> list[str]:
    try:
        tree = ast.parse(text, filename=name, feature_version=(3, 9))
    except SyntaxError as exc:
        return [f"{name}:{exc.lineno}: not valid Python 3.9 syntax: {exc.msg}"]
    problems: list[str] = []
    future = has_future_annotations(tree)
    seen: set[int] = set()
    in_annotation: set[int] = set()
    for ann in annotation_nodes(tree):
        for n in ast.walk(ann):
            in_annotation.add(id(n))
            if (not future and isinstance(n, ast.BinOp) and isinstance(n.op, ast.BitOr)
                    and n.lineno not in seen):
                seen.add(n.lineno)
                problems.append(f"{name}:{n.lineno}: `X | Y` annotation needs "
                                "`from __future__ import annotations` on Python 3.9")
    for n in ast.walk(tree):
        if (isinstance(n, ast.BinOp) and isinstance(n.op, ast.BitOr)
                and (is_type_like(n.left) or is_type_like(n.right)) and n.lineno not in seen
                and id(n) not in in_annotation):
            seen.add(n.lineno)
            problems.append(f"{name}:{n.lineno}: `X | Y` between types fails at run time on Python "
                            "3.9 unless it sits in an annotation under the __future__ import")
    return problems


def tracked_py(root: Path) -> list[Path]:
    out = subprocess.run(["git", "-C", str(root), "ls-files", "*.py"],
                         capture_output=True, text=True, check=True).stdout
    return [root / line for line in out.splitlines() if line]


def main(argv: list[str]) -> int:
    root = Path(argv[1]) if len(argv) > 1 else Path(__file__).resolve().parent.parent
    problems: list[str] = []
    files = tracked_py(root)
    for path in files:
        if path.exists():
            problems += check_source(path.read_text(encoding="utf-8"), str(path.relative_to(root)))
    for p in problems:
        print(f"  ✗ {p}")
    if problems:
        return 1
    print(f"  ✓ {len(files)} tracked .py files are Python 3.9 compatible")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))

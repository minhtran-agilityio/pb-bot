"""Tests for scripts/check_py39_compat.py (issue #192): 3.10-only syntax must not come back."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_py39_compat as compat  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent


def test_union_annotation_without_future_import_is_flagged():
    assert compat.check_source("def f(x: str | None): ...\n")
    assert compat.check_source("def f() -> int | None: ...\n")
    assert compat.check_source("x: int | None = None\n")


def test_union_annotation_with_future_import_passes():
    assert compat.check_source("from __future__ import annotations\ndef f(x: str | None): ...\n") == []


def test_runtime_none_union_flagged_even_with_future_import():
    assert compat.check_source("from __future__ import annotations\nT = int | None\n")


def test_runtime_type_unions_flagged_in_every_spelling():
    for src in ("isinstance(1, int | str)\n", "T = list[int] | dict\n", "T = dict | None\n",
                "import typing\nT = typing.Optional | Foo\n", "T = Foo | Bar\n"):
        assert compat.check_source("from __future__ import annotations\n" + src), src


def test_match_statement_flagged():
    assert compat.check_source("match 1:\n    case 1:\n        pass\n")


def test_int_bitor_passes():
    assert compat.check_source("y = 1 | 2\n") == []


def test_every_tracked_python_file_is_39_compatible():
    problems = []
    for path in compat.tracked_py(ROOT):
        if path.exists():
            problems += compat.check_source(path.read_text(encoding="utf-8"), str(path.relative_to(ROOT)))
    assert problems == []

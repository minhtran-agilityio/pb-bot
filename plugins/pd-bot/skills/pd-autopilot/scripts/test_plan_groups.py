"""Tests for plan_groups.py."""
import json
import subprocess
import sys
from pathlib import Path

SCRIPT = Path(__file__).with_name("plan_groups.py")


def run(data, *extra):
    return subprocess.run([sys.executable, str(SCRIPT), *extra], input=json.dumps(data),
                          capture_output=True, text=True)


def groups(r):
    return {x["n"]: x["group"] for x in json.loads(r.stdout)}


def test_single_ticket_is_group_one():
    r = run([{"n": 346, "deps": []}])
    assert r.returncode == 0 and groups(r) == {346: 1}


def test_independent_tickets_share_group_one():
    assert groups(run([{"n": 1, "deps": []}, {"n": 2}])) == {1: 1, 2: 1}


def test_longest_path_wins():
    r = run([{"n": 4, "deps": [2, 3]}, {"n": 3, "deps": [2]}, {"n": 2, "deps": []}])
    assert groups(r) == {2: 1, 3: 2, 4: 3}


def test_output_keeps_input_order():
    r = run([{"n": 9, "deps": [5]}, {"n": 5, "deps": []}])
    assert [x["n"] for x in json.loads(r.stdout)] == [9, 5]


def test_dep_outside_batch_is_ignored_with_note():
    r = run([{"n": 7, "deps": [99]}])
    assert r.returncode == 0 and groups(r) == {7: 1}
    assert "#99" in r.stderr and "closed" in r.stderr


def test_cycle_exits_2_naming_tickets():
    r = run([{"n": 1, "deps": [2]}, {"n": 2, "deps": [3]}, {"n": 3, "deps": [1]}])
    assert r.returncode == 2
    assert all(f"#{n}" in r.stderr for n in (1, 2, 3))


def test_input_file(tmp_path):
    f = tmp_path / "in.json"
    f.write_text(json.dumps([{"n": 1, "deps": []}, {"n": 2, "deps": [1]}]))
    r = subprocess.run([sys.executable, str(SCRIPT), "--input", str(f)],
                       capture_output=True, text=True)
    assert groups(r) == {1: 1, 2: 2}


def test_bad_json_exits_2():
    r = subprocess.run([sys.executable, str(SCRIPT)], input="nope", capture_output=True, text=True)
    assert r.returncode == 2


# --- paths: two tickets that touch the same file or directory never share a group -----------------

def test_same_file_runs_in_sequence_in_input_order():
    r = run([{"n": 1, "paths": ["src/a.py"]}, {"n": 2, "paths": ["src/a.py"]}])
    assert r.returncode == 0 and groups(r) == {1: 1, 2: 2}
    assert "#2" in r.stderr and "#1" in r.stderr and "src/a.py" in r.stderr


def test_directory_overlaps_a_file_inside_it():
    r = run([{"n": 1, "paths": ["src/"]}, {"n": 2, "paths": ["./src/deep/a.py"]}])
    assert groups(r) == {1: 1, 2: 2}


def test_a_name_prefix_is_not_an_overlap():
    r = run([{"n": 1, "paths": ["src/a"]}, {"n": 2, "paths": ["src/ab.py"]}])
    assert groups(r) == {1: 1, 2: 1} and r.stderr == ""


def test_disjoint_paths_and_missing_paths_stay_parallel():
    r = run([{"n": 1, "paths": ["a.py"]}, {"n": 2, "paths": ["b.py"]}, {"n": 3}])
    assert groups(r) == {1: 1, 2: 1, 3: 1}


def test_overlap_pushes_the_dependents_too():
    r = run([{"n": 1, "paths": ["a.py"]}, {"n": 2, "paths": ["a.py"]}, {"n": 3, "deps": [2]}])
    assert groups(r) == {1: 1, 2: 2, 3: 3}


def test_overlap_follows_the_dependency_order_not_the_input_order():
    r = run([{"n": 1, "deps": [2], "paths": ["a.py"]}, {"n": 2, "paths": ["a.py"]}])
    assert groups(r) == {2: 1, 1: 2}


def test_bad_paths_exit_2():
    assert run([{"n": 1, "paths": "a.py"}]).returncode == 2
    assert run([{"n": 1, "paths": [3]}]).returncode == 2


# --- --max-parallel: no group holds more tickets than the cap ----------------------------------------

def test_max_parallel_splits_a_group():
    r = run([{"n": n} for n in (1, 2, 3, 4, 5)], "--max-parallel", "2")
    assert groups(r) == {1: 1, 2: 1, 3: 2, 4: 2, 5: 3}
    assert "max-parallel" in r.stderr


def test_max_parallel_keeps_dependents_after_their_dependency():
    r = run([{"n": 1}, {"n": 2}, {"n": 3}, {"n": 4, "deps": [3]}], "--max-parallel", "2")
    g = groups(r)
    assert g == {1: 1, 2: 1, 3: 2, 4: 3}


def test_max_parallel_fills_a_free_slot_of_an_earlier_group():
    r = run([{"n": 1}, {"n": 2, "deps": [1]}, {"n": 3}, {"n": 4}], "--max-parallel", "2")
    assert groups(r) == {1: 1, 3: 1, 2: 2, 4: 2}


def test_max_parallel_must_be_positive():
    assert run([{"n": 1}], "--max-parallel", "0").returncode == 2


def test_no_option_and_no_paths_gives_the_old_result():
    r = run([{"n": 4, "deps": [2, 3]}, {"n": 3, "deps": [2]}, {"n": 2}, {"n": 8}])
    assert groups(r) == {2: 1, 3: 2, 4: 3, 8: 1} and r.stderr == ""


def test_dot_is_the_whole_repository_and_an_empty_path_exits_2():
    assert groups(run([{"n": 1, "paths": ["."]}, {"n": 2, "paths": ["x/y.py"]}])) == {1: 1, 2: 2}
    assert run([{"n": 1, "paths": [""]}]).returncode == 2

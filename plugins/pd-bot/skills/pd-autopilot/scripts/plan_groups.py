#!/usr/bin/env python3
"""Turn ticket dependencies into execution groups.

Input JSON (stdin, or --input FILE): [{"n": 1, "deps": [2, 3], "paths": ["src/a.py", "docs/"]}, ...]
Output JSON: [{"n": 1, "group": 1}, ...] in input order.
The groups run in order; the tickets of one group run in parallel. A ticket gets the first group
that comes after every dependency, that holds no ticket with an overlapping path, and that has a
free slot under --max-parallel. Without paths and without --max-parallel that is
1 + the longest dependency path inside the batch.

paths (optional) are the files and directories a ticket is expected to change, relative to the
repository root. Two paths overlap when they are equal or one is a directory that holds the other.
A ticket without paths overlaps nothing. Of two overlapping tickets, the one that depends on the
other goes later; otherwise the one later in the input goes later.

A dep outside the batch is ignored (a note goes to stderr): it must already be closed. Each ticket
that moved because of an overlap or the cap gets one note on stderr. A cycle exits 2 and names the
tickets.
"""
import argparse
import json
import sys


def norm_paths(n, raw):
    if raw is None:
        return []
    if not isinstance(raw, list) or not all(isinstance(p, str) for p in raw):
        raise TypeError(f"#{n}: paths must be a list of strings")
    out = []
    for p in raw:
        p = p.strip().replace("\\", "/")
        while p.startswith("./"):
            p = p[2:]
        p = p.strip("/")
        if not p:
            raise TypeError(f"#{n}: a path is empty; write . for the whole repository")
        out.append("" if p == "." else p)  # "" is the repository root: it overlaps every path
    return sorted(set(out))


def overlap(a, b):
    """The first pair of overlapping paths of two tickets, or None."""
    for x in a:
        for y in b:
            if x == y or not x or not y or y.startswith(x + "/") or x.startswith(y + "/"):
                return x or ".", y or "."
    return None


def plan(items, max_parallel=None):
    nums = [int(i["n"]) for i in items]
    batch = set(nums)
    deps, paths = {}, {}
    for i in items:
        n = int(i["n"])
        inside = []
        for d in i.get("deps") or []:
            d = int(d)
            if d == n:
                continue
            if d in batch:
                inside.append(d)
            else:
                print(f"plan_groups: #{n} depends on #{d}, outside the batch; "
                      f"ignored (it must already be closed)", file=sys.stderr)
        deps[n] = sorted(set(inside))
        paths[n] = norm_paths(n, i.get("paths"))
    depth, state = {}, {}

    def visit(n, path):
        if state.get(n) == 2:
            return depth[n]
        if state.get(n) == 1:
            cyc = path[path.index(n):] + [n]
            raise ValueError("dependency cycle: " + " -> ".join(f"#{x}" for x in cyc))
        state[n] = 1
        depth[n] = 1 + max((visit(d, path + [n]) for d in deps[n]), default=0)
        state[n] = 2
        return depth[n]

    for n in nums:
        visit(n, [])

    # Place the tickets in dependency order (then input order), each in the first group that fits.
    group, size = {}, {}
    for n in sorted(nums, key=lambda x: (depth[x], nums.index(x))):
        g = 1 + max((group[d] for d in deps[n]), default=0)
        why = []
        while True:
            hit = next(((m, overlap(paths[n], paths[m])) for m in group
                        if group[m] == g and overlap(paths[n], paths[m])), None)
            if hit:
                why.append(f"after #{hit[0]}: both touch {hit[1][0]}"
                           + ("" if hit[1][0] == hit[1][1] else f" / {hit[1][1]}"))
            elif max_parallel and size.get(g, 0) >= max_parallel:
                why.append(f"group {g} is full (--max-parallel {max_parallel})")
            else:
                break
            g += 1
        group[n] = g
        size[g] = size.get(g, 0) + 1
        if why:
            print(f"plan_groups: #{n} moved to group {g}: " + "; ".join(why), file=sys.stderr)
    return [{"n": n, "group": group[n]} for n in nums]


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--input", help="JSON file; default is stdin")
    ap.add_argument("--max-parallel", type=int, help="the most tickets one group may hold")
    args = ap.parse_args(argv)
    try:
        if args.max_parallel is not None and args.max_parallel < 1:
            raise ValueError("--max-parallel must be 1 or more")
        raw = open(args.input).read() if args.input else sys.stdin.read()
        out = plan(json.loads(raw), args.max_parallel)
    except ValueError as e:
        print(f"plan_groups: {e}", file=sys.stderr)
        return 2
    except (OSError, KeyError, TypeError) as e:
        print(f"plan_groups: bad input: {e}", file=sys.stderr)
        return 2
    print(json.dumps(out))
    return 0


if __name__ == "__main__":
    sys.exit(main())

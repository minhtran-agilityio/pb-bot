#!/usr/bin/env bash
# ship-worktree-cleanup.sh: the end-of-run worktree step of workflows/ship.js (P10).
#   ship-worktree-cleanup.sh --repo-dir <repo> --run-dir <dir> [--keep-all]
# It reads the worktrees of <repo> that live under <run-dir> (git worktree list), so the command names no
# stage dir. Without --keep-all (a run that succeeded), each stage worktree (detached, or on a scratch/
# branch: the group merge stage) is removed with `git worktree remove` (never --force), so one with changes
# or untracked files stays; one whose HEAD is on no other branch or remote branch (commits nobody
# published) also stays. A worktree on any other branch (a ticket branch) always stays. With --keep-all (a run that did not succeed) nothing is removed. A dir under
# <run-dir> that is not a worktree of <repo> is never touched. Then the run dir is removed if it is empty
# (rmdir, never rm -r).
# Output, one line each (tab-separated):
#   removed <dir>
#   kept <dir> <size> <reason>
#   run-dir removed|kept <dir>
#   command <the exact command that removes one kept worktree, paths shell-quoted>
#   error <why>                    (bad arguments or repo: nothing was read or removed)
# It always exits 0: cleanup never fails a run.
set -u
# A stop before any worktree is read prints one "error <why>" line on stdout, so the caller can tell
# "nothing to do" from "did not run".
err() { printf 'error\t%s\n' "$1"; exit 0; }
repo="" run="" all=""
while [ $# -gt 0 ]; do
  case "$1" in
    --repo-dir|--run-dir) [ $# -ge 2 ] || err "$1 needs a value"
      if [ "$1" = --repo-dir ]; then repo=$2; else run=$2; fi; shift 2 ;;
    --keep-all) all=1; shift ;;
    *) err "unknown argument $1" ;;
  esac
done
[ -n "$repo" ] && [ -n "$run" ] || err "--repo-dir and --run-dir are required"
git -C "$repo" rev-parse --git-dir >/dev/null 2>&1 || err "--repo-dir $repo is not a git checkout"
if [ ! -d "$run" ]; then printf 'run-dir\tkept\t%s\n' "$run"; exit 0; fi
runp=$(cd "$run" && pwd -P)
size() { LC_ALL=C du -sh "$1" 2>/dev/null | cut -f1; }
kept() { printf 'kept\t%s\t%s\t%s\n' "$1" "$(size "$1")" "$2"; printf 'command\tgit -C %q worktree remove %q\n' "$repo" "$1"; }

# path<TAB>kind<TAB>branch ref for each worktree under the run dir (kind: detached or branch).
wts=$(git -C "$repo" worktree list --porcelain | awk -v r="$runp/" '
  /^worktree / { p = substr($0, 10); d = "branch"; b = "" }
  /^branch / { b = substr($0, 8) }
  /^detached$/ { d = "detached" }
  /^$/ { if (index(p, r) == 1) print p "\t" d "\t" b; p = "" }
  END { if (p != "" && index(p, r) == 1) print p "\t" d "\t" b }')
seen=""
while IFS=$'\t' read -r d kind ref; do
  [ -n "$d" ] || continue
  seen="$seen"$'\n'"$d"
  if [ -n "$all" ]; then kept "$d" "the run did not succeed"; continue; fi
  # A stage worktree is detached, or on a scratch/ branch (the merge stage of a multi-ticket run). Any other
  # branch is a ticket branch, and it stays.
  case "$kind:$ref" in
    detached:*|branch:refs/heads/scratch/*) ;;
    *) kept "$d" "a ticket branch worktree"; continue ;;
  esac
  if [ -n "$(git -C "$d" status --porcelain 2>/dev/null)" ]; then kept "$d" "uncommitted changes or untracked files"; continue; fi
  sha=$(git -C "$d" rev-parse -q --verify HEAD 2>/dev/null) || { kept "$d" "no HEAD"; continue; }
  # The commits must be on a branch or remote branch other than the worktree's own scratch branch.
  if ! git -C "$repo" for-each-ref --contains "$sha" --format='%(refname)' refs/heads refs/remotes 2>/dev/null | grep -qvxF "$ref"; then
    kept "$d" "HEAD $sha is on no other branch"; continue
  fi
  if git -C "$repo" worktree remove "$d" 2>/dev/null; then printf 'removed\t%s\n' "$d"; else kept "$d" "git worktree remove refused"; fi
done <<< "$wts"
# Anything else under the run dir is not a worktree of this repo: report it, never touch it.
for d in "$runp"/*; do
  [ -d "$d" ] || continue
  case "$seen"$'\n' in *$'\n'"$d"$'\n'*) continue ;; esac
  printf 'kept\t%s\t%s\t%s\n' "$d" "$(size "$d")" "not a worktree of $repo"
done
if rmdir "$run" 2>/dev/null; then printf 'run-dir\tremoved\t%s\n' "$run"; else printf 'run-dir\tkept\t%s\n' "$run"; fi
exit 0

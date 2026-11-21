#!/usr/bin/env bash
# ship-worktree-cleanup.sh: the end-of-run worktree step of workflows/ship.js (P10).
#   ship-worktree-cleanup.sh --repo-dir <repo> --run-dir <dir> [--keep-all]
# It reads the worktrees of <repo> that live under <run-dir> (git worktree list), so the command names no
# stage dir. Without --keep-all (a run that succeeded), each DETACHED stage worktree is removed with
# `git worktree remove` (never --force), so one with changes or untracked files stays; one whose HEAD is on
# no branch or remote branch (commits nobody published) also stays. A worktree on a branch (a ticket
# branch) always stays. With --keep-all (a run that did not succeed) nothing is removed. A dir under
# <run-dir> that is not a worktree of <repo> is never touched. Then the run dir is removed if it is empty
# (rmdir, never rm -r).
# Output, one line each (tab-separated):
#   removed <dir>
#   kept <dir> <size> <reason>
#   run-dir removed|kept <dir>
#   command <the exact command that removes one kept worktree>
# It always exits 0: cleanup never fails a run.
set -u
repo="" run="" all=""
while [ $# -gt 0 ]; do
  case "$1" in
    --repo-dir) repo=${2-}; shift 2 ;;
    --run-dir) run=${2-}; shift 2 ;;
    --keep-all) all=1; shift ;;
    *) printf 'ship-worktree-cleanup: unknown argument %s\n' "$1" >&2; exit 0 ;;
  esac
done
if [ -z "$repo" ] || [ -z "$run" ] || ! git -C "$repo" rev-parse --git-dir >/dev/null 2>&1; then
  printf 'ship-worktree-cleanup: --repo-dir must be a git checkout and --run-dir must be set\n' >&2
  exit 0
fi
if [ ! -d "$run" ]; then printf 'run-dir\tkept\t%s\n' "$run"; exit 0; fi
runp=$(cd "$run" && pwd -P)
size() { LC_ALL=C du -sh "$1" 2>/dev/null | cut -f1; }
kept() { printf 'kept\t%s\t%s\t%s\n' "$1" "$(size "$1")" "$2"; printf 'command\tgit -C %s worktree remove %s\n' "$repo" "$1"; }

# path<TAB>detached|branch for each worktree under the run dir.
wts=$(git -C "$repo" worktree list --porcelain | awk -v r="$runp/" '
  /^worktree / { p = substr($0, 10); d = "branch" }
  /^detached$/ { d = "detached" }
  /^$/ { if (index(p, r) == 1) print p "\t" d; p = "" }
  END { if (p != "" && index(p, r) == 1) print p "\t" d }')
seen=""
while IFS=$'\t' read -r d kind; do
  [ -n "$d" ] || continue
  seen="$seen"$'\n'"$d"
  if [ -n "$all" ]; then kept "$d" "the run did not succeed"; continue; fi
  if [ "$kind" != detached ]; then kept "$d" "a ticket branch worktree"; continue; fi
  if [ -n "$(git -C "$d" status --porcelain 2>/dev/null)" ]; then kept "$d" "uncommitted changes or untracked files"; continue; fi
  sha=$(git -C "$d" rev-parse -q --verify HEAD 2>/dev/null) || { kept "$d" "no HEAD"; continue; }
  if [ -z "$(git -C "$repo" for-each-ref --contains "$sha" --format='%(refname)' refs/heads refs/remotes 2>/dev/null)" ]; then
    kept "$d" "HEAD $sha is on no branch"; continue
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

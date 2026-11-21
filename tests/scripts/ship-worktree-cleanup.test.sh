#!/usr/bin/env bash
# P10: plugins/pd-bot/scripts/ship-worktree-cleanup.sh removes the detached stage worktrees of a ship.js run
# that succeeded, with `git worktree remove` (no --force). It keeps a dirty one, one whose HEAD is on no
# branch, every ticket-branch worktree, and with --keep-all everything. Throwaway git repo with a bare remote.
source "$(dirname "$0")/../lib.sh"
S="$ROOT/plugins/pd-bot/scripts/ship-worktree-cleanup.sh"

W=$(mktemp -d); _CLEANUP_DIRS+=("$W")
W=$(cd "$W" && pwd -P)
git init -q --bare "$W/remote.git"
git init -q -b main "$W/repo"
R="$W/repo"; RUN="$W/.ship-repo/ms"
git -C "$R" -c user.email=t@t -c user.name=t commit -q --allow-empty -m init
git -C "$R" remote add origin "$W/remote.git"; git -C "$R" push -q origin main
mkdir -p "$R/.git/info"; echo 'target/' >> "$R/.git/info/exclude"         # build output is ignored
mk() { git -C "$R" worktree add -q "$@"; }
setup() {
  mkdir -p "$RUN"
  mk --detach "$RUN/review-r1" origin/main                                  # clean, detached
  mkdir -p "$RUN/review-r1/target"; printf 'build\n' > "$RUN/review-r1/target/out"   # ignored: does not block
  mk --detach "$RUN/merge-g1-check" origin/main                             # a stage name with merge in it
  mk --detach "$RUN/fix-dirty" origin/main; echo x > "$RUN/fix-dirty/new.txt"
  mk --detach "$RUN/fix-unpushed" origin/main
  git -C "$RUN/fix-unpushed" -c user.email=t@t -c user.name=t commit -q --allow-empty -m lost
  mk -b "feat/1-x$1" "$RUN/t1" origin/main                                  # the ticket branch
  mk -b "scratch/ms-merge-g1$1" "$RUN/merge-g1" origin/main                 # group merge stage, work landed
  mk -b "scratch/ms-merge-g2$1" "$RUN/merge-g2" origin/main                 # group merge stage, work only here
  git -C "$RUN/merge-g2" -c user.email=t@t -c user.name=t commit -q --allow-empty -m only-here
  mkdir -p "$RUN/stray"; echo keep > "$RUN/stray/f"                         # not a worktree
}
has() { printf '%s\n' "$out" | grep -qE "$1"; }

setup a
out=$(bash "$S" --repo-dir "$R" --run-dir "$RUN"); rc=$?
t "it exits 0"; [ "$rc" = 0 ] && pass || fail "rc=$rc out=$out"
t "a clean detached worktree (with ignored build output) is removed"
if has "^removed	$RUN/review-r1$" && [ ! -d "$RUN/review-r1" ] && ! git -C "$R" worktree list | grep -q review-r1; then pass; else fail "$out"; fi
t "a detached worktree whose name holds merge is found without naming it in the command"
if has "^removed	$RUN/merge-g1-check$"; then pass; else fail "$out"; fi
t "a worktree with untracked files is kept, with the reason"
if has "^kept	$RUN/fix-dirty	.*	uncommitted changes" && [ -f "$RUN/fix-dirty/new.txt" ]; then pass; else fail "$out"; fi
t "a worktree whose HEAD is on no branch is kept"
if has "^kept	$RUN/fix-unpushed	.*	HEAD .* is on no other branch" && [ -d "$RUN/fix-unpushed" ]; then pass; else fail "$out"; fi
t "the ticket-branch worktree is kept and sized"
if has "^kept	$RUN/t1	[0-9.]+[KMGT]?	a ticket branch worktree$" && [ -d "$RUN/t1" ]; then pass; else fail "$out"; fi
t "a clean scratch/ merge-stage worktree whose commits are on another branch is removed"
if has "^removed	$RUN/merge-g1$" && [ ! -d "$RUN/merge-g1" ]; then pass; else fail "$out"; fi
t "a scratch/ worktree whose commits are only on its own branch is kept"
if has "^kept	$RUN/merge-g2	.*	HEAD .* is on no other branch" && [ -d "$RUN/merge-g2" ]; then pass; else fail "$out"; fi
t "each kept worktree gets the exact removal command"
if has "^command	git -C $R worktree remove $RUN/t1$" && has "^command	git -C $R worktree remove $RUN/fix-dirty$"; then pass; else fail "$out"; fi
t "a dir that is not a worktree is reported and never touched"
if has "^kept	$RUN/stray	.*	not a worktree" && [ -f "$RUN/stray/f" ]; then pass; else fail "$out"; fi
t "the run dir is kept while it holds worktrees"
if has "^run-dir	kept	$RUN$" && [ -d "$RUN" ]; then pass; else fail "$out"; fi

# --keep-all (a run that did not succeed): nothing is removed.
for d in fix-dirty fix-unpushed t1 merge-g2; do git -C "$R" worktree remove --force "$RUN/$d"; done; rm -rf "$RUN"
setup b
out=$(bash "$S" --repo-dir "$R" --run-dir "$RUN" --keep-all)
t "--keep-all removes nothing and reports every worktree"
n=$(git -C "$R" worktree list | grep -c "$RUN/")
if [ "$n" = 7 ] && ! has '^removed' && has "^kept	$RUN/review-r1	.*	the run did not succeed$" && has "^kept	$RUN/merge-g1	"; then pass; else fail "n=$n $out"; fi

# A run dir left empty is removed (rmdir only).
mk --detach "$W/.ship-repo/r2/review-r1" origin/main
out=$(bash "$S" --repo-dir "$R" --run-dir "$W/.ship-repo/r2")
t "an emptied run dir is removed"
if has "^run-dir	removed	$W/.ship-repo/r2$" && [ ! -d "$W/.ship-repo/r2" ]; then pass; else fail "$out"; fi

t "a bad --repo-dir exits 0 with an error line on stdout and removes nothing"
out=$(bash "$S" --repo-dir "$W/nope" --run-dir "$RUN" 2>/dev/null); rc=$?
if [ "$rc" = 0 ] && has "^error	--repo-dir $W/nope is not a git checkout$" && [ -d "$RUN/t1" ]; then pass; else fail "rc=$rc out=$out"; fi

t "a flag with no value ends with an error line instead of looping"
out=$( (bash "$S" --repo-dir "$R" --run-dir & p=$!; ( sleep 5; kill $p 2>/dev/null ) & k=$!; wait $p; rc=$?; kill $k 2>/dev/null; exit $rc) ); rc=$?
if [ "$rc" = 0 ] && has '^error	--run-dir needs a value$'; then pass; else fail "rc=$rc out=$out"; fi

# A repo path with a space: the printed removal command quotes both paths.
mkdir -p "$W/my proj"; git -C "$R" worktree add -q -b feat/sp "$W/my proj/.ship/r4/t9" origin/main
out=$(bash "$S" --repo-dir "$R" --run-dir "$W/my proj/.ship/r4")
t "the removal command quotes a path with a space"
if has "^command	git -C $R worktree remove $W/my\\\\ proj/\.ship/r4/t9$"; then pass; else fail "$out"; fi

finish

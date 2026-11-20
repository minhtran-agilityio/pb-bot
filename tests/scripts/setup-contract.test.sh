#!/bin/bash
# Contract tests for the setup skill: its SKILL.md steps.
source "$(dirname "$0")/../lib.sh"

DIR="$ROOT/plugins/pd-bot/skills/pd-setup"
SKILL="$DIR/SKILL.md"

t "SKILL.md frontmatter: name setup, user-invoked only, declares allowed-tools"
fm=$(awk 'NR==1&&/^---$/{inb=1;next} inb&&/^---$/{exit} inb' "$SKILL")
if printf '%s\n' "$fm" | grep -q '^name: pd-setup$' && printf '%s\n' "$fm" | grep -q '^disable-model-invocation: true$' \
   && printf '%s\n' "$fm" | grep -qE '^allowed-tools:.*AskUserQuestion'; then pass; else fail "frontmatter: $fm"; fi

t "SKILL.md is at most 80 lines and under 600 words"
lines=$(wc -l < "$SKILL" | tr -d ' '); words=$(wc -w < "$SKILL" | tr -d ' ')
if [ "$lines" -le 80 ] && [ "$words" -lt 600 ]; then pass; else fail "lines=$lines words=$words"; fi

t "SKILL.md runs init-structure, merges deny, ignores specs/, creates no agent index or techstacks"
if grep -q '${CLAUDE_PLUGIN_ROOT}/scripts/init-structure.sh' "$SKILL" \
   && grep -q 'merge-deny.sh' "$SKILL" && grep -q '\.gitignore' "$SKILL" && grep -q 'specs/' "$SKILL" \
   && ! grep -q 'PROJECT.template' "$SKILL" && ! grep -q 'techstacks' "$SKILL"; then pass; else fail "a setup step is missing or a removed one is back"; fi

t "SKILL.md proposes ## Run and test before writing it and asks before deleting an old PROJECT.md"
flat2=$(tr '\n' ' ' < "$SKILL" | tr -s ' ')
if printf '%s' "$flat2" | grep -qF '## Run and test' && printf '%s' "$flat2" | grep -qF 'propose' \
   && printf '%s' "$flat2" | grep -qF 'only after the owner agrees' && printf '%s' "$flat2" | grep -qF 'ask before deleting'; then pass
else fail "the Run and test step is missing or does not ask first"; fi

t "SKILL.md step 4 names merge-guard.sh (fails closed, protected bases only) and offers no gh pr merge allow rule"
flat=$(tr '\n' ' ' < "$SKILL" | tr -s ' ')
if grep -qF 'merge-guard.sh' "$SKILL" && printf '%s' "$flat" | grep -qF 'blocks every merge into a protected base (main, master, release/*, the default branch and similar); it fails closed' \
   && printf '%s' "$flat" | grep -qF 'stay human-merged' \
   && ! grep -qF 'merge-allow.sh' "$SKILL" && ! grep -qF 'Bash(gh pr merge *)' "$SKILL" \
   && printf '%s' "$flat" | grep -qF 'Setup never writes `.claude/settings.local.json`'; then pass
else fail "the guard text is missing or an allow offer is still there"; fi

t "docs/hooks.md does not claim setup offers a gh pr merge allow rule through merge-allow.sh"
if ! grep -qF 'merge-allow.sh' "$ROOT/docs/hooks.md" && ! grep -qF 'can also offer one allow rule' "$ROOT/docs/hooks.md"; then pass
else fail "docs/hooks.md still describes the removed allow offer"; fi

t "SKILL.md tells the owner to add the gh pr merge allow rule themselves (setup writes none)"
if printf '%s' "$flat" | grep -qF 'the owner adds an allow rule for `gh pr merge` into `feat/<slug>*` to their own settings; setup never writes it'; then pass
else fail "setup does not say how auto-merge is permitted"; fi

t "every file the SKILL.md names under CLAUDE_SKILL_DIR exists"
ok=1
for f in $(grep -oE '\$\{CLAUDE_SKILL_DIR\}/[A-Za-z0-9_./-]+' "$SKILL" | sed 's#\${CLAUDE_SKILL_DIR}/##' | sort -u); do
  [ -f "$DIR/$f" ] || { ok=0; echo "        missing: $f"; }
done
[ "$ok" -eq 1 ] && pass || fail "a referenced file is missing"

t "the PROJECT.md template resource is gone and the init-structure templates exist"
if [ ! -e "$DIR/resources" ] && [ -f "$ROOT/plugins/pd-bot/templates/structure/specs-README.md" ] \
   && [ -x "$ROOT/plugins/pd-bot/scripts/init-structure.sh" ]; then pass; else fail "a setup resource is missing"; fi

t "implementer keeps a legacy agents/PROJECT.md fallback like test-runner"
if grep -q 'agents/PROJECT.md' "$ROOT/plugins/pd-bot/agents/implementer.md" 2>/dev/null; then pass; else fail "implementer.md has no agents/PROJECT.md fallback"; fi

t "setup allowed-tools includes Write and Edit (step 2 writes CLAUDE.md)"
if sed -n '1,/^---$/p;' "$ROOT/plugins/pd-bot/skills/pd-setup/SKILL.md" | grep -E '^allowed-tools:' | grep -q 'Write' \
   && grep -E '^allowed-tools:' "$ROOT/plugins/pd-bot/skills/pd-setup/SKILL.md" | grep -q 'Edit'; then pass; else fail "allowed-tools lacks Write or Edit"; fi

t "setup deletes the old PROJECT.md by an absolute CLAUDE_PROJECT_DIR path"
if grep -qF 'rm -- ${CLAUDE_PROJECT_DIR}/agents/PROJECT.md' "$ROOT/plugins/pd-bot/skills/pd-setup/SKILL.md"; then pass; else fail "rm of agents/PROJECT.md is not anchored to CLAUDE_PROJECT_DIR"; fi

finish

#!/bin/bash
# P11: the pd-orchestrate orchestrator removes the worker and reviewer workspaces of a finished ticket itself.
# Text contract: the qualifying conditions, the order (podbay-dev stop, then ws-rm), the refusals that keep a
# workspace, the ledger record and the final-report list. The commands were checked against
# `podbay-dev stop --help` ("Stop a session; its conversation is kept", `stop [OPTIONS] <NAME>`) and
# `podbay-dev ws-rm --help` ("Delete a workspace; refuses when work is not pushed", `ws-rm [OPTIONS] <PROJECT> <WS>`).
source "$(dirname "$0")/../lib.sh"
D="$ROOT/plugins/pd-bot/skills/pd-orchestrate"
SKILL="$D/SKILL.md"; POD="$D/references/podbay.md"; RUN="$D/references/run.md"; PROTO="$D/references/protocol.md"
# section <file> <heading>: the text of one "## <heading>" section.
section() { awk -v h="## $2" '$0==h{on=1;next} on&&/^## /{exit} on' "$1"; }
CLEAN=$(section "$POD" "Clean up")

t "orchestrate SKILL.md stays under 600 words"
[ "$(wc -w < "$SKILL")" -lt 600 ] && pass || fail "words=$(wc -w < "$SKILL")"

t "SKILL.md: the orchestrator removes a finished ticket's workspaces itself (podbay.md, Clean up)"
if grep -q 'workspace' "$SKILL" && grep -qi 'finished ticket' "$SKILL" && grep -q 'podbay.md`, Clean up' "$SKILL" \
   && ! grep -q 'delete one only when the owner says so' "$SKILL"; then pass; else fail "SKILL.md rule not updated"; fi

t "Clean up: a workspace qualifies only when the PR is MERGED or CLOSED, the receipt is in, and no other ticket uses it"
if printf '%s' "$CLEAN" | grep -q 'MERGED' && printf '%s' "$CLEAN" | grep -q 'CLOSED' \
   && printf '%s' "$CLEAN" | grep -q 'receipt' && printf '%s' "$CLEAN" | grep -qi 'no other ticket'; then pass; else fail "$CLEAN"; fi

t "Clean up: stop the session first, then ws-rm, in that order"
if printf '%s\n' "$CLEAN" | awk '/podbay-dev stop </{s=NR} /podbay-dev ws-rm </{r=NR} END{exit !(s && r && s<r)}'; then pass; else fail "order"; fi

t "Clean up: a refused ws-rm keeps the workspace, with the reason in the ledger; never forced or deleted by hand"
if printf '%s' "$CLEAN" | grep -qi 'refuses' && printf '%s' "$CLEAN" | grep -qi 'ledger' \
   && printf '%s' "$CLEAN" | grep -q -- '--force' && printf '%s' "$CLEAN" | grep -qi 'by hand'; then pass; else fail "$CLEAN"; fi

t "Clean up: never src, never a workspace a session outside the fleet uses"
if printf '%s' "$CLEAN" | grep -q '`src`' && printf '%s' "$CLEAN" | grep -qi 'outside'; then pass; else fail "$CLEAN"; fi

t "podbay.md: ws-rm is no longer only the owner's"
owner=$(section "$POD" "Commands you run only when the owner asks")
if ! printf '%s' "$owner" | grep -q '^| `podbay-dev ws-rm'; then pass; else fail "$owner"; fi

t "protocol.md: the ledger records a removed or kept workspace"
if section "$PROTO" "Ledger" | grep -q '"workspaces"' && section "$PROTO" "Ledger" | grep -q '"removed"' \
   && section "$PROTO" "Ledger" | grep -q '"kept"'; then pass; else fail "no workspaces record in the ledger"; fi

t "run.md: the final report lists each kept workspace with its reason"
if section "$RUN" "Final report to the owner" | grep -qi 'kept workspace' \
   && section "$RUN" "Final report to the owner" | grep -qi 'reason'; then pass; else fail "$(section "$RUN" "Final report to the owner")"; fi

finish

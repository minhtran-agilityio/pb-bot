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
OWNER=$(section "$POD" "Commands you run only when the owner asks")
LEDGER=$(section "$PROTO" "Ledger")
REPORT=$(section "$RUN" "Final report to the owner")

# A renamed heading would leave a section empty, and a negative check on it would pass: fail first instead.
t "the sections this contract reads exist and are not empty"
if [ -n "$CLEAN" ] && [ -n "$OWNER" ] && [ -n "$LEDGER" ] && [ -n "$REPORT" ]; then pass; else fail "a section heading changed"; fi

t "orchestrate SKILL.md stays under 600 words"
[ "$(wc -w < "$SKILL")" -lt 600 ] && pass || fail "words=$(wc -w < "$SKILL")"

t "SKILL.md: the orchestrator removes a finished ticket's workspaces itself (podbay.md, Clean up)"
if grep -q 'workspace' "$SKILL" && grep -qi 'finished ticket' "$SKILL" && grep -q 'podbay.md`, Clean up' "$SKILL" \
   && ! grep -q 'delete one only when the owner says so' "$SKILL"; then pass; else fail "SKILL.md rule not updated"; fi

t "Clean up: a workspace qualifies only when the PR of every ticket that used it is MERGED or CLOSED, the receipt is in, and no unfinished ticket uses it"
if printf '%s' "$CLEAN" | grep -q 'MERGED' && printf '%s' "$CLEAN" | grep -q 'CLOSED' \
   && printf '%s' "$CLEAN" | grep -q 'receipt' && printf '%s' "$CLEAN" | grep -qi 'every ticket in the ledger that used' \
   && printf '%s' "$CLEAN" | grep -qi 'not finished'; then pass; else fail "$CLEAN"; fi

t "Clean up: an advisor workspace stays while a worker may still fetch its prototype"
if printf '%s' "$CLEAN" | grep -qi 'advisor workspace' && printf '%s' "$CLEAN" | grep -qi 'prototype'; then pass; else fail "$CLEAN"; fi

t "Clean up: save the worker's ignored specs/, then stop the session, then ws-rm, in that order"
if printf '%s\n' "$CLEAN" | awk '/worker-specs/{w=NR} /podbay-dev stop </{s=NR} /podbay-dev ws-rm </{r=NR} END{exit !(w && s && r && w<s && s<r)}'; then pass; else fail "order"; fi

t "Clean up: a squash-merged ticket whose ws-rm refuses is kept and listed with the PR link"
if printf '%s' "$CLEAN" | grep -qi 'squash' && printf '%s' "$CLEAN" | grep -qi 'PR link'; then pass; else fail "$CLEAN"; fi

t "Clean up: a refused ws-rm keeps the workspace, with the reason in the ledger; never forced or deleted by hand"
if printf '%s' "$CLEAN" | grep -qi 'refuses' && printf '%s' "$CLEAN" | grep -qi 'ledger' \
   && printf '%s' "$CLEAN" | grep -q -- '--force' && printf '%s' "$CLEAN" | grep -qi 'by hand'; then pass; else fail "$CLEAN"; fi

t "Clean up: never src, never a workspace a session outside the fleet uses"
if printf '%s' "$CLEAN" | grep -q '`src`' && printf '%s' "$CLEAN" | grep -qi 'outside'; then pass; else fail "$CLEAN"; fi

t "podbay.md: ws-rm is no longer only the owner's"
if ! printf '%s' "$OWNER" | grep -q '^| `podbay-dev ws-rm'; then pass; else fail "$OWNER"; fi

t "protocol.md: the ledger records every use of a workspace and its removed or kept state"
if printf '%s' "$LEDGER" | grep -q '"workspaces"' && printf '%s' "$LEDGER" | grep -q '"uses"' \
   && printf '%s' "$LEDGER" | grep -q '"removed"' && printf '%s' "$LEDGER" | grep -q '"kept"'; then pass; else fail "no workspaces record in the ledger"; fi

t "run.md: the final report lists each kept workspace with its reason"
if printf '%s' "$REPORT" | grep -qi 'kept workspace' && printf '%s' "$REPORT" | grep -qi 'reason'; then pass; else fail "$REPORT"; fi

finish

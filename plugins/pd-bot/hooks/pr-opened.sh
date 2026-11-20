#!/usr/bin/env bash
# PostToolUse(Bash): after `gh pr create` printed a PR URL, remind the model to review the PR. Reminds only; never blocks.
command -v jq >/dev/null || exit 0
payload=$(cat)
cmd=$(printf '%s' "$payload" | jq -r '.tool_input.command // empty' 2>/dev/null) || exit 0
printf '%s' "$cmd" | grep -qE '\bgh[[:space:]]+pr[[:space:]]+create\b' || exit 0
pr=$(printf '%s' "$payload" | jq -r '.tool_response | tostring' 2>/dev/null | grep -oE 'github\.com/[^ "\\]+/pull/[0-9]+' | head -1 | grep -oE '[0-9]+$')
# The command text alone proves nothing: a grep or a scan for the words `gh pr create` holds them
# too, and a `gh pr create` that failed opened no PR. A PR was opened only when the output also
# holds a PR URL. A PR URL alone is not enough either (`gh pr edit` prints one).
[ -n "$pr" ] || exit 0
# The orchestrator session never reviews alone (session-role.sh decides who that is; any error there
# leaves the text for every other session).
role=$(bash "$(dirname "$0")/session-role.sh" --role 2>/dev/null </dev/null) || role=""
if [ "$(printf '%s' "$payload" | jq -r '.agent_id // empty')" ]; then
  text="PR #${pr:-?} was opened from a subagent. Return the PR number to the main session; subagents do not review."
elif [ "$role" = "orchestrator" ]; then
  text="PR #${pr:-?} was opened. This session is the orchestrator: do not run /code-review here. Hand the /code-review of this PR, then the intent-review, to another session (a Pod Bay reviewer session), and check its result before you report done or the owner merges."
else
  text="PR #${pr:-?} was opened. Next step: run /code-review on it now, then intent-review, before merge or reporting done. This applies when you finished a stopped or blocked workflow by hand: its review stage did not run."
fi
jq -n --arg t "$text" '{hookSpecificOutput:{hookEventName:"PostToolUse",additionalContext:$t}}'

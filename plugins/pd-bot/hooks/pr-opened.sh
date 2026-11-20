#!/usr/bin/env bash
# PostToolUse(Bash): after `gh pr create` printed a PR URL, remind the model to review the PR. Reminds only; never blocks.
command -v jq >/dev/null || exit 0
payload=$(cat)
cmd=$(printf '%s' "$payload" | jq -r '.tool_input.command // empty' 2>/dev/null) || exit 0
# `gh pr create` must be a command, not text inside an argument: split the command at && || ; |
# and at line ends, and take a part only when it starts with `gh pr create` (after an opening
# parenthesis and VAR=value assignments; also `url=$(gh pr create ...)`, `command gh ...` and a
# full path to gh). So `grep "gh pr create" ...` and
# `gh pr review --body "... gh pr create ..."` do not count.
# Known gaps: a script that runs `gh pr create` inside it gets no reminder; a quoted string or a
# heredoc with a separator or a line right before the words still counts.
# shellcheck disable=SC2016  # the $( and the backtick are regex text, not shell
CREATE_RE='^[[:space:]({]*([A-Za-z_][A-Za-z0-9_]*=[^[:space:]]*[[:space:]]+)*([A-Za-z_][A-Za-z0-9_]*=)?["$(`{[:space:]]*(command[[:space:]]+)?([^[:space:]"=]*/)?gh[[:space:]]+pr[[:space:]]+create([[:space:])]|$)'
printf '%s\n' "$cmd" | awk '{ gsub(/&&|\|\||;|\|/, "\n"); print }' \
  | grep -qE "$CREATE_RE" || exit 0
pr=$(printf '%s' "$payload" | jq -r '.tool_response | tostring' 2>/dev/null | grep -oE 'github\.com/[^ "\\]+/pull/[0-9]+' | head -1 | grep -oE '[0-9]+$')
# A `gh pr create` that failed opened no PR: the output must also hold a PR URL. A PR URL alone
# is not enough either (`gh pr edit` prints one).
[ -n "$pr" ] || exit 0
# The orchestrator session never reviews alone (session-role.sh decides who that is; any error there
# leaves the text for every other session). session-role.sh runs tmux: the timeout of this hook in
# hooks.json ends a hang, and then no reminder is added (fail open).
role=$(bash "$(dirname "$0")/session-role.sh" --role 2>/dev/null </dev/null) || role=""
if [ "$(printf '%s' "$payload" | jq -r '.agent_id // empty')" ]; then
  text="PR #${pr:-?} was opened from a subagent. Return the PR number to the main session; subagents do not review."
elif [ "$role" = "orchestrator" ]; then
  text="PR #${pr:-?} was opened. This session is the orchestrator: do not run /code-review here. Hand the /code-review of this PR, then the intent-review, to another session (a Pod Bay reviewer session), and check its result before you report done or the owner merges."
else
  text="PR #${pr:-?} was opened. Next step: run /code-review on it now, then intent-review, before merge or reporting done. This applies when you finished a stopped or blocked workflow by hand: its review stage did not run."
fi
jq -n --arg t "$text" '{hookSpecificOutput:{hookEventName:"PostToolUse",additionalContext:$t}}'

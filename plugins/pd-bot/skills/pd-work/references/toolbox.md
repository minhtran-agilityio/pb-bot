# Worker toolbox

Part of `pd-bot:pd-work`.

Reach for a tool when the situation matches. Skip one when it adds nothing, and note why in the
report when a reader would expect it.

| Situation | Tool |
|---|---|
| The ticket is vague, or two readings differ in what you would build | send `question` to the orchestrator with your recommendation; go on with the parts that do not depend on the answer |
| Wrong behavior, cause not yet known | Read `${CLAUDE_PLUGIN_ROOT}/skills/pd-diagnosing-bugs/SKILL.md` and follow it (the Skill tool cannot load that skill): a failing repro test comes first |
| You do not know how this part of the code or a library works | `pd-bot:pd-xia2`, or a `pd-bot:researcher` agent for a narrow lookup |
| More than one sound approach, or depth L | `pd-bot:pd-shape` in auto mode; write the design note; ask the `pd-bot:advisor` agent to review it before code |
| Work with more than two or three moving parts | `pd-bot:pd-writing-plans`, then `pd-bot:pd-implement` (one implementer per step, independent steps in parallel) |
| Your branch already holds a commit `Add acceptance tests for #<n>` | those tests are your target and you never edit their directory. If one is wrong or cannot pass for a reason outside your code, send `question` with the evidence; the orchestrator can have it repaired |
| The ticket is risky (protocol, last paragraph) | the `pd-bot:security-reviewer` agent on your diff before you ask for review; fix every critical and high finding. The reviewer runs its own security review; yours only saves a round |
| The same check failed twice after a fix | one question to the `pd-bot:advisor` agent; if still stuck, `escalate` with the attempts, their output and the advisor's answer |
| You chose something a reader might not expect | `pd-bot:pd-decision-record` |
| Your diff has a UI change (protocol, "Outcome contract") | attach a screenshot of the changed screen to the PR (`gh pr comment <n> --attach <file>`, before and after when the screen existed). If you cannot, write `Merge: human (no UI screenshot attached)` in the PR body and go on. The screenshot never decides pass or fail |
| A test run of more than a few seconds | the `pd-bot:test-runner` agent, so your context stays small |

You may start agents of your own and run independent work in parallel. Choose each agent's model
and effort from the task.

## After you reported or escalated

Stay available. A `findings` message from the orchestrator reopens the ticket: fix, get a new
receipt for the new head, and report again; the 3-round limit starts again for each such cycle. A
second `assign` for the same ticket replaces your brief: it may name an approach and a branch to
fetch from another checkout, which you may then do. After `escalate`, wait for the orchestrator's
`answer` or `assign`.

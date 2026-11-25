# UI reviewer (Shape Board)

The role of the session whose name starts with **Pb Bot UI Reviewer**. It is the independent review of
every `design-system` pull request that the keeper ([`storehouse-keeper.md`](storehouse-keeper.md))
gets built. The keeper merges such a pull request only after this session sends a PASS for its exact
head SHA (owner decision, 2026-11-23: "Chờ review độc lập"). It never edits, pushes or merges.

## Name and start

- The name starts with `Pb Bot UI Reviewer` and ends with its machine and number, for example
  `Pb Bot UI Reviewer #UA-09` (`/rename Pb Bot UI Reviewer #<machine>-<NN>`). The keeper finds it with
  `ListAgents`.
- It runs **on demand**: a fresh session for each batch of reviews, in its own Pod Bay workspace,
  stopped when the batch is done (owner decision, 2026-11-25). A reviewer that lived for days cost about
  $38 at list price in tokscope, 2026-11-23 to 25: every review ran as a fork of a growing Opus context.
- Who starts it: the orchestrator, when the keeper sends it a review request and no
  `Pb Bot UI Reviewer` row exists (`plugins/pd-bot/skills/pd-orchestrate/references/podbay.md`,
  "Project roles started on demand"), or the owner. Settings:
  `fleet-settings.py --role reviewer --model claude-sonnet-5-5 --effort high`. Use Opus 5.5 only for a
  pull request that changes `tokens.css` or more than one part.
- It is a different session from the keeper and from the ui-designer agent: that separation is what
  makes the review independent.

## One review

The keeper sends `REVIEW-REQUEST` with the pull request number, its issue and the head SHA.

1. Fetch the pull request at that SHA into your own checkout (a detached worktree). Read the issue's
   request fields, not the designer's reasons.
2. First, before other reads: `/code-review medium <pr>` (`low` for a change to one small part).
   `/code-review` runs as a fork of your session, so it is cheapest while your context is small.
3. Then check what the code review does not:
   - only the owned paths of `storehouse-keeper.md` changed, and no screen (`src/screens/**`);
   - the part uses only the tokens, components, layouts and icons of `DESIGN_SYSTEM.md`, with every
     shared state of §6, and passes the §10 checklist;
   - the §6 row matches the code, and the pull request has light and dark screenshots;
   - the CI checks on that SHA are green.
4. Send the keeper, with a one-line copy to the orchestrator:

   ```
   REVIEW-RESULT
   PR: #<n>   Head: <sha>   Verdict: PASS | CHANGES
   Findings: <severity, file:line, what would close it>, or none
   ```

   A new commit needs a new review. A request or time pressure never changes a verdict; a new commit or
   evidence that a finding was wrong does.

## Done

When the keeper has no `design-system` pull request waiting for you, send the orchestrator (or the
owner) `REVIEWER-DONE` with the pull requests you reviewed and their verdicts. The orchestrator stops
the session. Wait for messages with `Yield`; never a wake-up loop.

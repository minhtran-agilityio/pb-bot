# The owner answers first

The owner must be able to explain and defend each big decision. So for a big decision and for every
how-decision, the owner answers first and only then sees your recommendation.

A decision is big when it is hard to reverse (a data schema or storage, a public API or contract, an
external dependency, security or auth, running cost, behavior users see) or your confidence is low.
Keep at most 3. Tell the owner in one line: "I will ask you first on: X, Y, Z." The owner's list
wins.

## A big decision

1. Ask in plain text, not with `AskUserQuestion`. One decision per message. Give no options and no
   recommendation: "What would you choose for X, and why?" Then wait.
2. Give your recommendation and its reason. Say where it differs from the owner's answer.
3. Give the strongest argument against your own recommendation.
4. Ask for the final choice with `AskUserQuestion`: the owner's answer first, then your
   recommendation if it differs (else the option the counter-argument favors), then "more facts".
   Label each option by its content only. Add no tag such as "(Recommended)": the owner has already
   heard your view.

When the owner has no first answer, record `Owner first answer: none`.

## A how-decision (brainstorm)

1. Ask the owner to sketch their own approach in 2-3 sentences. Then wait.
2. Present 2-3 viable approaches with trade-offs and a recommendation. Say which one is closest to
   the sketch and what the sketch did not cover. The sketch is the owner's first answer: go on from
   step 2 above.
3. After the choice, write 2-3 assumptions that would make the design wrong if they were true, each
   with a way to check it. They go into the brief.

## Record

Each big decision goes into the brief's `Decisions` section with the owner's first answer. For a
ticket, also add three lines after `Decision:` in `specs/<slug>/decision.md`:

```text
- Owner first answer: <what the owner said before seeing your answer>
- Agent recommendation: <yours>
- Counter-argument: <the argument against your recommendation>
```

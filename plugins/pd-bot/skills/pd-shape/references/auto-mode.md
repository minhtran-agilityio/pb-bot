# Auto mode


For a caller that cannot answer (`Mode: auto`). A prototype needs an owner: in auto mode, stop and
say so.

- Never ask. Return each recommended answer in `decisions[]` (`{title, context, options, decision,
  why, consequences, how_to_reverse, confidence, needs_review}`).
- Set `needs_review: true` only for a question the owner left unanswered. Otherwise follow the
  caller's rule for `needs_review`.
- Depth M and L: write `specs/<slug>/design.md` from `references/design-template.md`. At depth L also
  return the design as one decision with `stage: design`.
- Write no ADR file and no comment unless the caller asks.
- Set `needs_advisor: true` with `advisor_question` when you are BLOCKED, have low confidence, or a
  decision affects other tickets or a public contract. Do not spawn agents
  (`docs/adr/0001-workflow-capabilities.md`).

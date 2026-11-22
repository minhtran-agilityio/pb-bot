# Brief template

One brief per feature, in `specs/<slug>/brief.md`. It is local scratch: `pd-tickets` copies what
each ticket needs into the ticket, so a worker on a clean clone never needs this file. Keep each
section short. Drop a section that has nothing to say, except `Out of scope`.

```markdown
# <feature name>

Source: <prototype path, issue or "owner text"> · Date: <YYYY-MM-DD>

## Goal
<who uses it, which problem it solves, what success looks like, in 1-3 sentences>

## Scope
- <each thing this version does, as visible behavior>

## Out of scope
- <each thing this version does not do>

## Rules
- <business rules, limits, calculations>

## Edge cases and errors
- <case> -> <expected behavior>

## Data and permissions
- <what is stored, where it comes from, who can see or change it>

## Non-functional
- <speed, security, supported devices>

## Priority
1. <what comes first>

## Decisions
- <decision> -- why: <reason>. Owner first answer: <text | none>.

## Assumptions that would break this
- <assumption> -- check: <how>

## ADRs
- <docs/adr/NNNN-*.md, or none>
```

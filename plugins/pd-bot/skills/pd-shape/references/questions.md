# Question format

The owner reads each question in a small terminal tab.

- ONE decision per question, at most 25 words; `header` is the ticket number (or the slug) plus a word,
  for example `#397 scope`. Shape one ticket at a time. When the caller batches tickets, each header
  names its ticket.
- Ask the owner for decisions only. Do not ask how to implement or test: that belongs to the plan.
- When `AskUserQuestion` is not available, write numbered text instead: one block per decision, one
  option per line, your recommendation first.
- 2 to 4 options. Your recommendation first, its label ending in `(Recommended)`, worded so that
  "yes" accepts it. At most 4 questions per `AskUserQuestion` call; the most blocking first.
- Evidence goes in the option `preview`: `references/review-layout.md`.

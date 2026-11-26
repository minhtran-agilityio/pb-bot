---
type: system
title: "Knowledge Store: ADRs, Solutions and Specs"
description: How pd-bot keeps one store per record type - ADRs in docs/adr written by write_adrs.py, lessons in docs/solutions built by the compound skill and an index script, and gitignored specs/ scratch scaffolded by init-structure.sh.
tags: [adr, docs-solutions, specs, compound, decision-record, scripts]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-26T15:02:34.678Z
sources:
  - id: openwiki-source-6a68fd38298620902ead151b
    resource: repo://plugins/pd-bot/scripts/init-structure.sh
  - id: openwiki-source-47a6f5d73e87e267312a9080
    resource: repo://plugins/pd-bot/scripts/rebuild_solution_index.py
  - id: openwiki-source-227c0f5115cc0fd5955c7cca
    resource: repo://plugins/pd-bot/scripts/write_adrs.py
  - id: openwiki-source-22322c1136b84139e5cf0017
    resource: repo://plugins/pd-bot/skills/pd-compound/SKILL.md
  - id: openwiki-source-5b68070498993bdaa0d7a174
    resource: repo://plugins/pd-bot/skills/pd-decision-record/SKILL.md
  - id: openwiki-source-0ab18e428f2c387aba54c229
    resource: repo://plugins/pd-bot/templates/structure/specs-README.md
generated: { by: "claude-code", at: "2026-11-26T15:02:34.678Z" }
---

# Knowledge Store: ADRs, Solutions and Specs

pd-bot keeps **one store per record type** and never mixes them:

| Store | Holds | Tracked in git | Written by |
|---|---|---|---|
| `docs/adr/` | Durable decisions and depth-L designs (an ADR with `stage: design`) | yes | `write_adrs.py` only, never by hand or by an agent directly |
| `docs/solutions/` | Solved problems and patterns (bugs, knowledge, failures) | yes | the `pd-compound` skill, with `rebuild_solution_index.py` |
| `specs/<slug>/` | Per-ticket working files | **no** (gitignored scratch) | skills along the ticket's path |

<!-- openwiki: broken internal link [/openwiki/workflows/ticket-to-pr-skill-pipeline.md] link "/openwiki/workflows/ticket-to-pr-skill-pipeline.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
`CLAUDE.md` adds that a solutions entry older than 30 days (`confirmed_at`) should be treated as possibly stale, that `docs/solutions/INDEX.md` is the starting point and `critical-patterns.md` is read when planning, and that `REVIEW.md` rules for the reviewer session are never edited inside a ticket PR. The workflow that fills these stores is in [Ticket-to-PR Skill Pipeline](/openwiki/workflows/ticket-to-pr-skill-pipeline.md).

## Decisions: `pd-decision-record` and `write_adrs.py`

Every autonomous stage that decides something on its own returns a `decisions[]` list (fields `ticket`, `stage`, `title`, `context`, `options`, `decision`, `why`, `consequences`, `how_to_reverse`, `confidence`, `needs_review`, `adr`, `decided_by`, optional `owner_confirmed` and `date`). Agents never write ADR files. The final ADR stage saves the list as `{"run": ..., "decisions": [...]}` and runs `scripts/write_adrs.py`, which **assigns numbers in code** and applies the rules, so agents do not filter.

**Which decisions get a file.** A file is written only if all hold: `adr` is true, `options` lists at least two real options (not `n/a`), the title does not start with `Boundary: ` or `UAT not run`, and the `stage` is before implement (exactly one of `intake`, `grilling`, `intent`, `research`, `design`, `plan`, `plan-review`). A decision that otherwise qualifies but comes from a later stage, or has no stage, is **late**: no file, `needs_review` is forced true even if the owner confirmed it, and the PR table says "late decision, no file". `needs_review` and `confidence` never create a file; `confidence` only marks a guess in the table. Files are always born `accepted`.

**De-duplication and supersession.** Before writing, duplicates (same ticket and either the same title slug or the same non-empty context) are merged, keeping the highest confidence; `needs_review` is OR-ed and `owner_confirmed` AND-ed. A decision whose ticket already has a live ADR with the same title slug or context *and* the same decision text is skipped as "already recorded (ADR NNNN)". If the decision text changed, a new ADR is written whose context begins "Supersedes ADR N." and the old ADR is rewritten with `status: superseded` and `superseded_by`. Run-level decisions (ticket `run`) repeat across runs and are never matched; a placeholder non-numeric ticket is skipped only on an identical decision and never supersedes.

**Failure behavior.** The script exits 2 on bad input, on a template placeholder it cannot fill, or when a file would be overwritten (files are opened with exclusive create). Supersede targets are planned and validated before anything is written; the new ADR is written *before* the old one is marked, so a failure leaves a loud error naming the file to mark by hand rather than a dangling `superseded_by`. A repo's own `docs/adr/TEMPLATE.md` wins over the built-in template, and a legacy `{needs_review}` line in it is dropped with a warning.

<!-- openwiki: broken internal link [/openwiki/pod-shape/overview.md] link "/openwiki/pod-shape/overview.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
**Outputs.** Stdout is a JSON list of written files. `--table-out` writes the markdown table of every merged decision for the PR's `## Decisions` section (the ADR column reads `ADR file`, `already recorded`, `late decision, no file`, `owner-confirmed` or `table only`), `--needs-out` writes `## Needs your decision` listing every `needs_review` decision, and `--render-section` renders the PR section from the script's JSON output. The same script is used by `pd-compound` for its decision track (with `stage` set to `compound`) and by Shape Board when the owner publishes an ADR from a grill ([Shape Board App](/openwiki/pod-shape/overview.md)).

## Lessons: `pd-compound` and the index

After a session that produced something reusable, `pd-compound` runs four passes in the current session (context analyzer, solution extractor, decision extractor, then a related-docs pass using the analyzer's module and tags). It emits only *complete* tracks: **bug** (problem, root cause, fix; the root cause comes from the confirmed hypothesis in `specs/<slug>/diagnosis.md` when present), **knowledge** (pattern, how to use), **decision** (routed to `docs/adr/`, not `docs/solutions/`) and **failure** (symptom, wrong approach, why it failed, correct approach). Each solution record needs an `applicable_when` field, which is the primary discovery sentence future agents scan. Overlap rules: a high-overlap match (same module and at least two tags) is a *second occurrence*, so the existing record is updated, marked with a `**Recurred:** <date>` line and promoted in `critical-patterns.md`, and a path-specific learning gets a *proposed* path-scoped rule that a human must approve; moderate overlap gets a new numbered file. `critical` severity needs all three of: multiple features or layers affected, 30 or more minutes wasted if unknown, and generalizable.

`scripts/rebuild_solution_index.py` is the authority for `docs/solutions/INDEX.md` and is run at the end of every compound run, even one that wrote nothing. It rebuilds the whole index from frontmatter (`problem_type`, `severity`, `tags`, `applicable_when`, `confirmed_at`) rather than appending, so renamed or deleted files cannot leave orphan rows. It skips `INDEX.md` and `critical-patterns.md`, groups entries by top-level category, and sorts each group by `confirmed_at` descending. With `--check` it fails if any solution lacks `applicable_when` or if the committed index differs from a fresh build (the date line is masked), and `--dry-run` prints without writing. Compound never writes to `CLAUDE.md`; it proposes an addition if the knowledge base is not referenced there.

## Specs scratch and scaffolding

Every ticket, at every depth, gets `specs/<slug>/` with five flow files: `intent.md` (intake), `design.md` (shape at depth M and L, writing-plans at S), `decision.md` (every deciding step), `plan.md` and `summary.md` (finishing the branch); `research-brief.md` and `diagnosis.md` exist when xia2 or diagnosing-bugs run. Each starts with a `Ticket · Depth · Date` line and keeps every heading at every depth. Files link by ID (`AC<n>` and `D<n>`). `specs/` is gitignored and local; a worker session writes its own checkout's files, and a step running in `Mode: auto` returns text instead of writing a file. Anything durable must also go to `docs/adr/` or `docs/solutions/`.

<!-- openwiki: broken internal link [/openwiki/testing/test-suite-and-ci.md] link "/openwiki/testing/test-suite-and-ci.md" is root-absolute, which no real consumer resolves against the repository root (not a coding agent reading the page, not GitHub's Markdown renderer, not a local viewer); use a path relative to this file instead. Fix the href or restore the target, then delete this comment. -->
`plugins/pd-bot/scripts/init-structure.sh` bootstraps a repo (run by `/pd-bot:pd-setup`): it copies four templates (`specs/README.md` and the `docs/solutions` README, INDEX and critical-patterns) from `templates/structure/`, **create-if-missing only**, printing `created` or `exists` per file and always exiting 0. `--root` targets another directory for tests. See [Test Suite, Lints and CI](/openwiki/testing/test-suite-and-ci.md) for the tests of these scripts (`test_write_adrs.py`, `test_rebuild_solution_index.py`, `init-structure.test.sh`, `compound-contract.test.sh`).

---
ticket: P3
stage: design
status: accepted
confidence: high
decided_by: owner
date: 2026-11-20
---

# 0003. Decisions are tracked again in docs/adr, numbered from 0001

## Context

The repository was imported without its docs folder: `.gitignore` held the line /docs/, so `docs/adr/` did not exist in a fresh clone, while CLAUDE.md names `docs/adr/` as the store of decisions and cites ADR numbers of the old repository. A decision made in a ticket had no tracked place.

## Options

- Track `docs/adr/` again and start at 0001; the rest of docs stays ignored (chosen)
- Copy the old ADRs from another checkout and keep their numbers
- Keep docs ignored and record decisions only in the pull request

## Decision

`.gitignore` ignores /docs/* and re-includes /docs/adr/. New ADRs start at 0001 and are written by `plugins/pd-bot/scripts/write_adrs.py`. The old ADRs are not copied. `scripts/check-md-links.sh` checks the links of the tracked ADR files like every tracked markdown file, and `scripts/lint-adr-paths.sh` checks the paths they cite.

## Why

A decision record that is not in the repository is lost for every other clone and for a reviewer. Starting at 0001 keeps the new store free of records that describe files of the old repository.

## Consequences

ADR numbers that CLAUDE.md and the skills cite from the old repository (for example 0041 and 0043) do not resolve to a file here; the new numbers do not continue that series. Other folders under docs stay local.

## How to reverse

Restore the /docs/ line in `.gitignore` and remove the files with git rm --cached.

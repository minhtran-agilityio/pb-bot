# Changelog

All notable changes to this harness are recorded here. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions are tracked in the root `VERSION`
file. Versions change only in the release PR (see the release section of `AGENTS.md`); the bump (**patch** for fixes and docs,
**minor** for a new skill or hook or a changed contract, **major** for a breaking change) is chosen when releasing.

## [Unreleased]

- `merge-guard.sh` no longer treats the letters `gh` inside a word (such as "light") as the `gh` command, and its wrapper denial names the workaround (a script file run by a path without the word). A script run next to real merge text stays denied (P1).

## [0.1.0] — 2026-11-20

- Initial release of the `harness` plugin: skills, agents, hooks and the `autopilot-run` ship workflow.

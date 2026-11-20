# Changelog

All notable changes to this harness are recorded here. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions are tracked in the root `VERSION`
file. Versions change only in the release PR (see the release section of `AGENTS.md`); the bump (**patch** for fixes and docs,
**minor** for a new skill or hook or a changed contract, **major** for a breaking change) is chosen when releasing.

## [Unreleased]

- `merge-guard.sh`: the wrapper denial names the workaround (put the command in a script file and run it by a path without the word). Prose that mentions merge next to `bash x.sh` stays denied: two security reviews broke both fixes tried; the bypasses they found are now regression tests (P1).

## [0.1.0] — 2026-11-20

- Initial release of the `harness` plugin: skills, agents, hooks and the `autopilot-run` ship workflow.

# Changelog

All notable changes to this harness are recorded here. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions are tracked in the root `VERSION`
file. Versions change only in the release PR (see the release section of `AGENTS.md`); the bump (**patch** for fixes and docs,
**minor** for a new skill or hook or a changed contract, **major** for a breaking change) is chosen when releasing.

## [Unreleased]

- `merge-guard.sh` no longer blocks a command that only mentions a merge in text next to a plain `bash <script>.sh` run, or has the letters `gh` inside a word such as "light" before the word merge. `bash -c`, a script on stdin, `source`, `eval`, `xargs` and any call that could write the script stay denied (P1).

## [0.1.0] — 2026-11-20

- Initial release of the `harness` plugin: skills, agents, hooks and the `autopilot-run` ship workflow.

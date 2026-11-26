# Files

- [Plugin Hook System](hook-system.md) - How the pd-bot plugin wires shell hooks into Claude Code sessions, and how the rules, orchestrator-role, PR-review reminder and worktree-cleanup hooks decide what to do and when to stay silent.
- [Safety Guards: merge-guard, secrets-scan, stop-evidence](safety-guards-merge-secrets-evidence.md) - Decision logic, threat model and limits of the three hooks that can stop an agent action (merge-guard.sh, secrets-scan.sh, stop-evidence.sh), plus the permissions.deny rules that back them.

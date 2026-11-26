# Files

- [Shape Board Design System and Shared Tokens](design-system-rules.md) - The enforced UI contract for Shape Board (tokens, component tiers, icon map, banned patterns), the keeper / ui-designer request flow for new shared parts, the grep gate that checks it, and how Pod Board stays in sync with the same tokens.
- [Shape Board App](overview.md) - Architecture of Shape Board (apps/pod-shape), a personal macOS Tauri 2 + React app that imports Claude prototypes, grills them through a sandboxed `claude -p` loop running the pd-shape skill, and stores sessions, briefs and ADR drafts in SQLite.

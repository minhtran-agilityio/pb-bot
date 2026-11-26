# Files

- [Pod Board Alerts, Build and Signing](alerts-and-build-signing.md) - The pure alert state machine in alerts.rs (new waits, reminders, finishes, Do Not Disturb, sound and flash), the build-mac.sh signing-identity logic that keeps macOS Local Network permission across rebuilds (ADR 0006), the one-installed-copy rule, and the tests that pin them.
- [Pod Board Menu Bar App](overview.md) - Architecture of Pod Board, a Tauri 2 observer app that polls podbay-dev and local Claude session files, merges them into an allow-listed model of sessions by state, and presents a board window, menu bar popover and notifications, with strict privacy sanitising.

# Preflight before triage

Step 1 runs `bash ${CLAUDE_SKILL_DIR}/scripts/preflight.sh --repo-dir <checkout dir>` before anything else.

Run it exactly as written, as its own Bash call, with nothing appended: no echo of the exit code and no
chain with other commands. The tool result already shows the exit code. The merge guard denies a shell call
next to the letters g-h, then p-r, then a dollar sign: "preflight" and "myprojects" hold the letters, so an
appended exit-code echo gets the step denied.

- Exit 0 (`preflight: ok`): go on.
- Exit 1 and the only line is `missing: specs/ ignore`: add the line `/specs/` to `.gitignore` in the
  checkout, go on, and say so in the step 7 summary. This is the only edit the skill makes in the
  owner's checkout.
- Exit 1 with any other `missing:` line: show every `missing:` line to the owner and stop before triage.

The script checks that git, gh, jq and python3 are in PATH, that gh is signed in, and that specs/ is
ignored. It does not check a system package that only a build shows as missing, or a tool version.
Stage agents handle those with the dependency rule of the ship workflow.

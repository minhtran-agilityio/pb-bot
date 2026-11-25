# UI evidence on the PR

A UI change is a diff that touches a `.tsx`, `.jsx`, `.vue`, `.svelte`, `.css`, `.scss` or `.html` file, or a ticket
of kind FE or FE+BE. A PR with a UI change needs before and after screenshots of each changed screen, on the PR
(after only, when the screen is new). There is no escape: when no screenshot can be made, ask the owner.
Make them from the running app, or with a headless browser and mock data when the app cannot run here.
Before you publish, look at each image: no real personal data, secrets, emails or private session text.
On a public repository, use mock data. Check `gh --version`, then attach after `gh pr create`:
- gh 2.99.0 or newer: `gh pr comment <n> --attach <file>`.
- Older gh: commit the images to the `pr-evidence` branch under `pr-<n>/` with git plumbing (no checkout;
  `git hash-object -w`, `git mktree`, `git commit-tree`, `git update-ref`), push it, and post one comment with
  a before/after table. Pin each image URL to that commit:
  `https://raw.githubusercontent.com/<owner>/<repo>/<sha>/pr-<n>/<file>`. Never commit screenshots to the ticket branch.
Check that one image URL returns HTTP 200. Skip this step when the caller says a later step
attaches the evidence. A screenshot is evidence for the reviewer; it never decides pass or fail.

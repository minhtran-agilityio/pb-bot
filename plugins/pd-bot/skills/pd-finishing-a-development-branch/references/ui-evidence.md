# UI evidence on the PR

A UI change is a diff that touches a `.tsx`, `.jsx`, `.vue`, `.svelte`, `.css`, `.scss` or `.html` file, or a ticket
of kind FE or FE+BE. A PR with a UI change needs before and after screenshots of each changed screen, on the PR
(after only, when the screen is new). A screen counts as changed when it renders a touched file, also when
nothing visible changed. There is no escape: when no screenshot can be made, ask the owner.
Make them from the running app, or with a headless browser and mock data when the app cannot run here.
Before you publish, look at each image: no real personal data, secrets, emails or private session text.
On a public repository, use mock data. Check `gh --version`, then attach after `gh pr create`:
- gh 2.99.0 or newer: `gh pr comment <n> --attach <file>`.
- Older gh: commit the images to the `pr-evidence` branch under `pr-<n>/` with git plumbing (no checkout), push
  without force, and post one comment with a before/after table. Never commit screenshots to the ticket branch.
  Add on top of the current tip, so the push is a fast-forward (use absolute paths for the files; it runs in bash and zsh):
  ```bash
  PARENT=""
  if git fetch -q origin pr-evidence; then
    PARENT=$(git rev-parse FETCH_HEAD)   # the branch may not exist yet: then PARENT stays empty
  fi
  export GIT_INDEX_FILE=$(mktemp -u)
  if [ -n "$PARENT" ]; then
    git read-tree "$PARENT"
  fi
  git update-index --add --cacheinfo 100644,$(git hash-object -w <file>),pr-<n>/<name>   # once per file
  T=$(git write-tree)
  rm -f "$GIT_INDEX_FILE"
  unset GIT_INDEX_FILE
  if [ -n "$PARENT" ]; then
    C=$(git commit-tree "$T" -p "$PARENT" -m "Evidence for PR #<n>")
  else
    C=$(git commit-tree "$T" -m "Evidence for PR #<n>")
  fi
  git push origin "${C}:refs/heads/pr-evidence"
  ```
  On a non-fast-forward push, fetch and run it again. Never force-push. Pin each image URL to `$C`:
  `https://raw.githubusercontent.com/<owner>/<repo>/<C>/pr-<n>/<name>`.
Check that one image URL returns HTTP 200. Skip this step when the caller says a later step
attaches the evidence. A screenshot is evidence for the reviewer; it never decides pass or fail.

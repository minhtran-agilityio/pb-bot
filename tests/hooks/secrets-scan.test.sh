#!/bin/bash
# Contract tests for hooks/secrets-scan.sh: blocks `git commit` with a secret or a .env staged;
# passes clean commits and every other command.
source "$(dirname "$0")/../lib.sh"

H=secrets-scan.sh
COMMIT=$(json_cmd 'git commit -m x')

R=$(new_repo $H)
stage "$R" src/app.py 'KEY = "sk-abcdefghijklmnopqrstuvwxyz123456"'
t "staged fake API key blocks a git commit (exit 2)"
run_hook "$R" $H "$COMMIT"
if [ "$RC" = 2 ] && printf '%s' "$OUT" | grep -q 'BLOCKED'; then pass; else fail "rc=$RC out=$OUT"; fi

t "git -C . commit is also blocked"
run_hook "$R" $H "$(json_cmd 'git -C . commit -m x')"
if [ "$RC" = 2 ]; then pass; else fail "rc=$RC"; fi

t "the same staged secret does not block a non-commit command"
run_hook "$R" $H "$(json_cmd 'git status')"
if [ "$RC" = 0 ]; then pass; else fail "rc=$RC out=$OUT"; fi

R=$(new_repo $H)
stage "$R" src/app.py 'print("hello")'
t "clean staged change passes"
run_hook "$R" $H "$COMMIT"
if [ "$RC" = 0 ]; then pass; else fail "rc=$RC out=$OUT"; fi

R=$(new_repo $H)
stage "$R" .env 'FOO=bar'
t "staged .env file blocks a git commit"
run_hook "$R" $H "$COMMIT"
if [ "$RC" = 2 ] && printf '%s' "$OUT" | grep -q '\.env'; then pass; else fail "rc=$RC out=$OUT"; fi

R=$(new_repo $H)
stage "$R" tests/fixture.py 'KEY = "sk-abcdefghijklmnopqrstuvwxyz123456"'
t "a placeholder secret under tests/ is not scanned"
run_hook "$R" $H "$COMMIT"
if [ "$RC" = 0 ]; then pass; else fail "rc=$RC out=$OUT"; fi

t "a malformed payload fails open"
run_hook "$R" $H 'not json'
if [ "$RC" = 0 ]; then pass; else fail "rc=$RC"; fi

# ---- #189 B1: the changes a commit WILL include, not only what is staged now ----
FAKE='KEY = "sk-abcdefghijklmnopqrstuvwxyz123456"'

# base_repo [file content...]: a repo with one clean committed file src/app.py
base_repo() {
  local r; r=$(new_repo $H)
  stage "$r" src/app.py 'print("hello")'
  git -C "$r" commit -qm init
  echo "$r"
}

R=$(base_repo)
printf '%s\n' "$FAKE" > "$R/src/new.py"
t "git add . && git commit blocks a secret in an untracked file"
run_hook "$R" $H "$(json_cmd 'git add . && git commit -m x')"
assert_rc 2

t "git add <path> && git commit blocks a secret in that path"
run_hook "$R" $H "$(json_cmd 'git add src/new.py && git commit -m x')"
assert_rc 2

t "git add -A; git commit (semicolon) blocks"
run_hook "$R" $H "$(json_cmd 'git add -A; git commit -m x')"
assert_rc 2

t "git add of a clean path does not block on a secret that stays out of the commit"
run_hook "$R" $H "$(json_cmd 'git add src/app.py && git commit -m x')"
assert_rc 0

t "a commit with nothing added ignores an untracked secret"
run_hook "$R" $H "$(json_cmd 'git commit -m x')"
assert_rc 0

t "the hook does not touch the real index"
if [ -z "$(git -C "$R" diff --cached --name-only)" ]; then pass; else fail "index changed: $(git -C "$R" diff --cached --name-only)"; fi

R=$(base_repo)
printf '%s\n' "$FAKE" > "$R/src/app.py"
t "git commit -a blocks a secret in a modified tracked file"
run_hook "$R" $H "$(json_cmd 'git commit -a -m x')"
assert_rc 2
t "git commit -am blocks"
run_hook "$R" $H "$(json_cmd 'git commit -am x')"
assert_rc 2
t "git commit --all blocks"
run_hook "$R" $H "$(json_cmd 'git commit --all -m x')"
assert_rc 2
t "git commit -m x src/app.py (path form) blocks"
run_hook "$R" $H "$(json_cmd 'git commit -m x src/app.py')"
assert_rc 2
t "git commit -m 'fix -a flag' is not read as -a"
run_hook "$R" $H "$(json_cmd "git commit -m 'fix -a flag'")"
assert_rc 0
t "git commit without -a ignores an unstaged modification"
run_hook "$R" $H "$(json_cmd 'git commit -m x')"
assert_rc 0

R=$(base_repo)
git -C "$R" rm -q --cached src/app.py
t "git commit -a on a deleted file passes"
run_hook "$R" $H "$(json_cmd 'git commit -am x')"
assert_rc 0

# ---- git -C <dir> and cd <dir> scan the named repo ----
R=$(base_repo)
O=$(new_repo)
stage "$O" src/app.py "$FAKE"
t "git -C <other repo> commit scans the other repo (secret staged there)"
run_hook "$R" $H "$(json_cmd "git -C $O commit -m x")"
assert_rc 2
t "cd <other repo> && git commit scans the other repo"
run_hook "$R" $H "$(json_cmd "cd $O && git commit -m x")"
assert_rc 2
t "git -C <other repo> add . && git commit scans the other repo"
O2=$(new_repo); printf '%s\n' "$FAKE" > "$O2/a.py"
run_hook "$R" $H "$(json_cmd "git -C $O2 add . && git -C $O2 commit -m x")"
assert_rc 2

R=$(base_repo)
stage "$R" src/leak.py "$FAKE"
O=$(new_repo); stage "$O" src/ok.py 'print(1)'
t "git -C <clean repo> commit passes although the hook's own cwd has a staged secret"
run_hook "$R" $H "$(json_cmd "git -C $O commit -m x")"
assert_rc 0
t "git -C <missing dir> commit passes (git itself fails)"
run_hook "$R" $H "$(json_cmd "git -C /nonexistent-dir-189 commit -m x")"
assert_rc 0

# ---- #189 B2: wider secret patterns ----
expect_block() { # expect_block <name> <file> <content>
  local r; r=$(new_repo $H); stage "$r" "$2" "$3"
  t "blocked: $1"
  run_hook "$r" $H "$COMMIT"; assert_rc 2
}
expect_pass() {
  local r; r=$(new_repo $H); stage "$r" "$2" "$3"
  t "passes: $1"
  run_hook "$r" $H "$COMMIT"; assert_rc 0
}
expect_block "sk-ant-api03 key"        src/a.py  'K = "sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789"'
expect_block "sk-proj key"             src/a.py  'K = "sk-proj-abcdefghijklmnopqrstuvwxyz0123456789"'
expect_block "GitHub ghp_ token"       src/a.py  'T = "ghp_abcdefghijklmnopqrstuvwxyz0123456789"'
expect_block "github_pat_ token"       src/a.py  'T = "github_pat_abcdefghijklmnopqrstuvwxyz0123456789"'
expect_block "glpat- token"            src/a.py  'T = "glpat-abcdefghijklmnopqrst"'
expect_block "Slack xoxb- token"       src/a.py  'T = "xoxb-1234567890-abcdefghij"'
expect_block "Stripe sk_live key"      src/a.py  'T = "sk_live_abcdefghijklmnop"'
expect_block "RSA private key"         src/k.txt '-----BEGIN RSA PRIVATE KEY-----'
expect_block "OPENSSH private key"     src/k.txt '-----BEGIN OPENSSH PRIVATE KEY-----'
expect_block "AWS_SECRET_ACCESS_KEY=abcd" cfg/aws.sh 'AWS_SECRET_ACCESS_KEY=abcd'
expect_block "aws_secret_access_key in an ini" cfg/credentials 'aws_secret_access_key = abcd'
expect_block "export API_KEY=<unquoted>" cfg/env.sh 'export API_KEY=abcdefghijkl'
expect_block "unquoted SERVICE_TOKEN=..." cfg/env.sh 'SERVICE_TOKEN=abcdefghijkl'
expect_block ".env.local file"         .env.local 'FOO=bar'
expect_block ".env.production file"    .env.production 'FOO=bar'
expect_block "nested config/.env.local" config/.env.local 'FOO=bar'
expect_block "prod.env file"           config/prod.env 'FOO=bar'

# existing exclusions and passing cases keep passing
expect_pass "an example env file"      .env.example 'FOO=bar'
expect_pass "a placeholder in a *.example file" cfg/aws.sh.example 'AWS_SECRET_ACCESS_KEY=abcd'
expect_pass "a secret in a .md file"   README.md 'export API_KEY=abcdefghijkl'
expect_pass "a secret under docs/"     docs/setup.txt 'K = "sk-proj-abcdefghijklmnopqrstuvwxyz0123456789"'
expect_pass "a secret under tests/"    tests/fix.py 'T = "ghp_abcdefghijklmnopqrstuvwxyz0123456789"'
expect_pass "a hyphenated word that contains sk-" src/a.py 'name = "risk-assessment-framework-documentation"'
expect_pass "a keyword argument that reads a value" src/a.py 'client(api_key=settings.api_key_value_for_service)'
expect_pass "an env var that references another" cfg/env.sh 'export API_KEY=$OTHER_KEY'
expect_pass "a private key mentioned in prose without the marker" src/a.py '# the private key lives in the vault'


# ---- security review of #189: redirects, $VAR directories, uncertain cd, --gpg-sign, objects ----
R=$(base_repo)
printf '%s\n' "$FAKE" > "$R/src/app.py"
for c in 'git commit -am x 2>&1' 'git commit -a -m x > /dev/null' 'git commit -m x -a 2>&1 || true' \
         'git commit -a -m x &>/dev/null' 'git commit -a -F - <<EOF
msg
EOF' 'git commit -a --gpg-sign -m x' 'git commit -a -S -m x'; do
  t "-a still scanned with: $(printf '%s' "$c" | head -1)"
  run_hook "$R" $H "$(jq -n --arg c "$c" '{tool_input:{command:$c}}')"; assert_rc 2
done

R=$(base_repo); printf '%s\n' "$FAKE" > "$R/src/new.py"
t "git add . 2>&1 && git commit blocks"
run_hook "$R" $H "$(json_cmd 'git add . 2>&1 && git commit -m x')"; assert_rc 2
t "a redirect on git add of a clean path does not fall back to staging everything"
run_hook "$R" $H "$(json_cmd 'git add src/app.py 2>&1 && git commit -m x')"; assert_rc 0

R=$(base_repo); stage "$R" src/leak.py "$FAKE"
t 'git -C "$PWD" commit (unknown directory) scans the current directory'
run_hook "$R" $H "$(jq -n '{tool_input:{command:"git -C \"$PWD\" commit -m x"}}')"; assert_rc 2
O=$(new_repo); stage "$O" src/ok.py 'print(1)'
t "(cd <other>); git commit still scans the current directory"
run_hook "$R" $H "$(json_cmd "(cd $O); git commit -m x")"; assert_rc 2
t "false && cd <other>; git commit still scans the current directory"
run_hook "$R" $H "$(json_cmd "false && cd $O; git commit -m x")"; assert_rc 2

R=$(base_repo)
printf '%s\n' "$FAKE" > "$R/src/app.py"
before=$(find "$R/.git/objects" -type f | wc -l)
run_hook "$R" $H "$(json_cmd 'git commit -am x')"
after=$(find "$R/.git/objects" -type f | wc -l)
t "the scan writes no object into the repository"
if [ "$before" = "$after" ]; then pass; else fail "objects $before -> $after"; fi

R=$(new_repo $H); stage "$R" "café/.env" 'FOO=bar'
t "a .env under a non-ASCII directory is blocked"
run_hook "$R" $H "$COMMIT"; assert_rc 2


finish

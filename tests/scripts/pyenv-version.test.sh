#!/bin/bash
# ensure_pyenv (tests/lib.sh) keys the shared venv by python major.minor (#192): sessions with a
# different python3 on PATH get different venvs and never delete each other's.
source "$(dirname "$0")/../lib.sh"

fake_python() { # fake_python <major.minor>: a dir with a python3 that reports that version and fails `-m venv`
  local d; d=$(mktemp -d); _CLEANUP_DIRS+=("$d")
  printf '#!/bin/bash\n[ "$1" = -m ] && exit 1\necho %s\n' "$1" > "$d/python3"
  chmod +x "$d/python3"
  echo "$d"
}

dir_for() { # dir_for <fakebin> <tmp>: the PYENV_DIR ensure_pyenv picks
  PATH="$1:$PATH" TMPDIR="$2" bash -c 'source "'"$ROOT"'/tests/lib.sh"; ensure_pyenv >/dev/null 2>&1; echo "$PYENV_DIR"'
}

tmp=$(mktemp -d); _CLEANUP_DIRS+=("$tmp")
a=$(dir_for "$(fake_python 3.9)" "$tmp")
b=$(dir_for "$(fake_python 3.10)" "$tmp")

t "a different python version gets its own venv directory"
[ -n "$a" ] && [ -n "$b" ] && [ "$a" != "$b" ] && pass || fail "3.9 -> '$a', 3.10 -> '$b'"

t "the venv directory name carries the version"
case "$a|$b" in *pd-bot-tests-venv-3.9*pd-bot-tests-venv-3.10*) pass ;; *) fail "'$a' '$b'" ;; esac

t "a venv for another version is left alone"
mkdir -p "$a/bin"; : > "$a/marker"
dir_for "$(fake_python 3.10)" "$tmp" >/dev/null
[ -e "$a/marker" ] && pass || fail "the 3.9 venv was removed"

finish

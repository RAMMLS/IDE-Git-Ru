#!/usr/bin/env sh
set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TEMP_ROOT="$(mktemp -d 2>/dev/null || mktemp -d -t aura-test)"
LOCAL_REPO="$TEMP_ROOT/local"
REMOTE_REPO="$TEMP_ROOT/remote"
CLONE_REPO="$TEMP_ROOT/clone"
AURA_CMD="$(command -v aura || true)"

if [ -z "$AURA_CMD" ]; then
  AURA_CMD="$HOME/.cargo/bin/aura"
fi

if [ ! -x "$AURA_CMD" ]; then
  printf "[ERROR] aura executable not found in PATH or %s.\n" "$HOME/.cargo/bin/aura"
  printf "Run install-aura.sh first.\n"
  exit 1
fi

assert_contains() {
  output="$1"
  needle="$2"
  message="$3"

  if ! printf '%s' "$output" | grep -F -- "$needle" >/dev/null 2>&1; then
    printf "%s\nExpected to find: %s\nActual output:\n%s\n" "$message" "$needle" "$output"
    exit 1
  fi
}

assert_true() {
  if [ "$1" != "true" ]; then
    printf "%s\n" "$2"
    exit 1
  fi
}

cleanup() {
  if [ -d "$TEMP_ROOT" ]; then
    rm -rf "$TEMP_ROOT"
  fi
}

trap cleanup EXIT

mkdir -p "$LOCAL_REPO" "$REMOTE_REPO" "$CLONE_REPO"

printf "== Init repositories ==\n"
"$AURA_CMD" init "$LOCAL_REPO"
"$AURA_CMD" init "$REMOTE_REPO"
"$AURA_CMD" init "$CLONE_REPO"

printf "== Initial commit ==\n"
printf '' > "$LOCAL_REPO/main.c"
status_before_add="$($AURA_CMD -C "$LOCAL_REPO" status 2>&1)"
assert_contains "$status_before_add" "untracked:" "Expected an untracked file before add"

"$AURA_CMD" -C "$LOCAL_REPO" add -A
commit_output="$($AURA_CMD -C "$LOCAL_REPO" commit -m 'initial commit' 2>&1)"
assert_contains "$commit_output" "main" "The first commit should create branch main"

printf "== Modify and diff ==\n"
printf 'int main() { return 0; }' > "$LOCAL_REPO/main.c"
diff_output="$($AURA_CMD -C "$LOCAL_REPO" diff 2>&1)"
assert_contains "$diff_output" "+int main() { return 0; }" "Diff should show the added line"
"$AURA_CMD" -C "$LOCAL_REPO" add main.c
"$AURA_CMD" -C "$LOCAL_REPO" commit -m 'update main.c'

printf "== Branch and switch ==\n"
"$AURA_CMD" -C "$LOCAL_REPO" branch feature
"$AURA_CMD" -C "$LOCAL_REPO" switch feature
printf 'void feature() {}' > "$LOCAL_REPO/feature.c"
"$AURA_CMD" -C "$LOCAL_REPO" add -A
"$AURA_CMD" -C "$LOCAL_REPO" commit -m 'add feature.c'

if [ ! -f "$LOCAL_REPO/feature.c" ]; then
  printf "feature.c should exist on branch feature\n"
  exit 1
fi

"$AURA_CMD" -C "$LOCAL_REPO" switch main
if [ -f "$LOCAL_REPO/feature.c" ]; then
  printf "feature.c should disappear after switching back to main\n"
  exit 1
fi

printf "== Untracked protection ==\n"
printf 'local only' > "$LOCAL_REPO/feature.c"
status_with_untracked="$($AURA_CMD -C "$LOCAL_REPO" status 2>&1)"
assert_contains "$status_with_untracked" "untracked:" "Status should report the local untracked file"

if printf '%s' "$status_with_untracked" | grep -F "unstaged:" >/dev/null 2>&1; then
  printf "An untracked file must not appear in unstaged as added\n"
  exit 1
fi

set +e
switch_error="$($AURA_CMD -C "$LOCAL_REPO" switch feature 2>&1)"
switch_exit_code=$?
set -e
if [ "$switch_exit_code" -eq 0 ]; then
  printf "Expected switch to fail because it would overwrite an untracked file\n"
  exit 1
fi
assert_contains "$switch_error" "untracked" "Expected an error about overwriting an untracked file"
rm -f "$LOCAL_REPO/feature.c"

printf "== Remote, push and pull ==\n"
"$AURA_CMD" -C "$LOCAL_REPO" remote add origin "$REMOTE_REPO"
"$AURA_CMD" -C "$LOCAL_REPO" push origin main
"$AURA_CMD" -C "$CLONE_REPO" remote add origin "$REMOTE_REPO"
"$AURA_CMD" -C "$CLONE_REPO" pull origin main

if [ ! -f "$CLONE_REPO/main.c" ]; then
  printf "main.c should appear in clone after pull\n"
  exit 1
fi

pull_again="$($AURA_CMD -C "$CLONE_REPO" pull origin main 2>&1)"
assert_contains "$pull_again" "Already up to date." "A repeated pull should say the branch is already up to date"

printf "\nAll Aura automated checks passed.\n"

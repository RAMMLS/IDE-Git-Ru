#!/usr/bin/env sh
set -eu

CARGO_BIN="$HOME/.cargo/bin"
PROFILE_BEGIN="# >>> aura path setup >>>"
PROFILE_END="# <<< aura path setup <<<"
PROFILE_BLOCK="$PROFILE_BEGIN\nexport PATH=\"$CARGO_BIN:\$PATH\"\n$PROFILE_END"

shell_profiles="
$HOME/.profile
$HOME/.bashrc
$HOME/.bash_profile
$HOME/.zshrc
$HOME/.zprofile
"

update_profile() {
  profile_path="$1"
  if [ ! -f "$profile_path" ]; then
    mkdir -p "$(dirname "$profile_path")"
    printf '%s\n' "$PROFILE_BLOCK" > "$profile_path"
    return
  fi

  if grep -F "$PROFILE_BEGIN" "$profile_path" >/dev/null 2>&1; then
    return
  fi

  printf '\n%s\n' "$PROFILE_BLOCK" >> "$profile_path"
}

for profile in $shell_profiles; do
  if [ -n "$profile" ]; then
    update_profile "$profile"
  fi

done

printf "[INFO] Added %s to PATH in shell profile files if needed.\n" "$CARGO_BIN"
printf "[INFO] New shells will load Cargo binaries once the profile is sourced.\n"

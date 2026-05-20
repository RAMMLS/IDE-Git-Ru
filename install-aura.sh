#!/usr/bin/env sh
set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VCS_CORE_DIR="$SCRIPT_DIR/vcs-core"
CARGO_BIN="$HOME/.cargo/bin"
AURA_EXE="$CARGO_BIN/aura"
SHELL_SETUP_SH="$SCRIPT_DIR/install-aura-shell.sh"
VERIFY_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t aura-install)"

printf "\n==========================================\n"
printf "  Aura installer\n"
printf "==========================================\n\n"

if [ ! -f "$VCS_CORE_DIR/Cargo.toml" ]; then
  printf "[ERROR] %s was not found.\n" "$VCS_CORE_DIR/Cargo.toml"
  printf "Make sure install-aura.sh is run from the IDE-Git-Ru project root.\n"
  exit 1
fi

if [ ! -f "$SHELL_SETUP_SH" ]; then
  printf "[ERROR] %s was not found.\n" "$SHELL_SETUP_SH"
  printf "The shell integration helper must be located next to install-aura.sh.\n"
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  printf "[INFO] cargo was not found. Trying to install Rust toolchain via rustup...\n"
  if command -v rustup >/dev/null 2>&1; then
    rustup toolchain install stable
  else
    if ! command -v curl >/dev/null 2>&1; then
      printf "[ERROR] curl is required to install rustup. Install Rust manually from https://rustup.rs/\n"
      exit 1
    fi

    printf "[INFO] Downloading rustup installer...\n"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  fi
fi

if [ -f "$HOME/.cargo/env" ]; then
  # shellcheck source=/dev/null
  . "$HOME/.cargo/env"
fi

if ! command -v cargo >/dev/null 2>&1; then
  printf "[ERROR] cargo is still unavailable in the current session.\n"
  printf "Close the terminal, open it again, and rerun install-aura.sh.\n"
  exit 1
fi

if [ ! -d "$CARGO_BIN" ]; then
  mkdir -p "$CARGO_BIN"
fi

export PATH="$CARGO_BIN:$PATH"

printf "[INFO] Installing local aura binary...\n"
cargo install --path "$VCS_CORE_DIR" --bin aura --force

if [ ! -f "$AURA_EXE" ]; then
  printf "[ERROR] Installation finished, but %s was not found.\n" "$AURA_EXE"
  exit 1
fi

printf "[INFO] Installing shell profile integration...\n"
sh "$SHELL_SETUP_SH"

printf "[INFO] Verifying aura launch from the installed binary...\n"
if ! "$AURA_EXE" help >/dev/null 2>&1; then
  printf "[ERROR] aura binary did not launch correctly from %s\n" "$AURA_EXE"
  exit 1
fi

printf "[INFO] Verifying aura launch from a fresh shell session...\n"
if ! sh -lc "PATH='$CARGO_BIN':\$PATH; aura help >/dev/null 2>&1" >/dev/null 2>&1; then
  printf "[WARN] aura is not available in a fresh shell session yet.\n"
  printf "[WARN] Reload your shell or open a new terminal window.\n"
else
  printf "[INFO] aura is available from a fresh shell session.\n"
fi

printf "\n[OK] Aura was installed successfully.\n"
printf "[OK] Executable file: %s\n" "$AURA_EXE"
printf "[OK] Global shell integration was written to your shell profile files.\n"
printf "[OK] New shells will see aura via ~/.cargo/bin in PATH.\n\n"
printf "If the aura command is still unavailable in the current terminal, open a new terminal or run:\n"
printf "  . ~/.cargo/env\n\n"
"$AURA_EXE" help

rm -rf "$VERIFY_DIR"
exit 0

#!/usr/bin/env sh
set -eu

# Check home directory is writable before doing anything
if [ ! -w "$HOME" ]; then
    echo "Error: HOME directory '$HOME' is not writable by $(whoami)." >&2
    echo "Fix with: sudo chown -R $(whoami):$(whoami) $HOME" >&2
    exit 1
fi

if command -v apt-get >/dev/null 2>&1; then
    sudo apt-get update
    sudo apt-get install -y curl build-essential pkg-config
elif command -v dnf >/dev/null 2>&1; then
    sudo dnf install -y curl gcc make pkg-config
elif command -v pacman >/dev/null 2>&1; then
    sudo pacman -Sy --needed curl base-devel pkgconf
elif command -v brew >/dev/null 2>&1; then
    brew install curl pkg-config
else
    echo "No supported package manager found. Install curl, a C build toolchain, and Rust manually." >&2
fi

if ! command -v cargo >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    . "$HOME/.cargo/env"
elif [ -f "$HOME/.cargo/env" ]; then
    . "$HOME/.cargo/env"
fi

cargo build --release

INSTALL_DIR="$HOME/.local/bin"
INSTALL_BIN="$INSTALL_DIR/splinterparty"
mkdir -p "$INSTALL_DIR"
cp "target/release/splinterparty" "$INSTALL_BIN"
chmod +x "$INSTALL_BIN"

echo
echo "Splinterparty installed to $INSTALL_BIN."
echo "Run setup with:"
echo "  $INSTALL_BIN setup"
echo
echo "Then install/start the user service with:"
echo "  ./service.sh install"

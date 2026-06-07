#!/usr/bin/env sh
set -eu

# Splinterparty installer
# Installs dependencies, builds release binary, copies it to ~/.local/bin,
# and makes helper scripts executable when present.

if [ "$(id -u)" -eq 0 ]; then
    echo "Error: do not run this script with sudo/root." >&2
    echo "Run it as your normal user: ./install.sh" >&2
    exit 1
fi

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
    # shellcheck disable=SC1090
    . "$HOME/.cargo/env"
fi

cargo build --release

INSTALL_DIR="$HOME/.local/bin"
BINARY_SRC="target/release/splinterparty"
BINARY_DST="$INSTALL_DIR/splinterparty"

if [ ! -f "$BINARY_SRC" ]; then
    echo "Error: built binary not found at $BINARY_SRC" >&2
    echo "Check that Cargo.toml builds a binary named 'splinterparty'." >&2
    exit 1
fi

mkdir -p "$INSTALL_DIR"
cp "$BINARY_SRC" "$BINARY_DST"
chmod 755 "$BINARY_DST"

# Make helper scripts executable if they exist in the project directory.
[ -f ./service.sh ] && chmod +x ./service.sh
[ -f ./install.sh ] && chmod +x ./install.sh

# Make sure ~/.local/bin is usable in future shells.
case ":${PATH}:" in
    *":$INSTALL_DIR:"*) ;;
    *)
        echo
        echo "Note: $INSTALL_DIR is not currently in PATH."
        echo "You can still run Splinterparty with: $BINARY_DST"
        ;;
esac

echo
echo "Splinterparty installed to $BINARY_DST."
echo "Run setup with:"
echo "  $BINARY_DST setup"
echo
echo "Then install/start the user service with:"
echo "  ./service.sh install"
echo
echo "Do not run service.sh with sudo. It is a user service."

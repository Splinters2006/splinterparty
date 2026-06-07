#!/usr/bin/env sh
set -eu

if command -v apt-get >/dev/null 2>&1; then
    sudo apt-get update
    sudo apt-get install -y curl build-essential pkg-config
elif command -v dnf >/dev/null 2>&1; then
    sudo dnf install -y curl gcc make pkg-config
elif command -v pacman >/dev/null 2>&1; then
    sudo pacman -Sy --needed curl base-devel pkgconf
else
    echo "No supported package manager found. Install curl, a C build toolchain, and Rust manually." >&2
fi

if ! command -v cargo >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    export PATH="$HOME/.cargo/bin:$PATH"
fi

cargo build --release

echo
echo "Splinterparty installed."
echo "Run setup with:"
echo "  cargo run --release -- setup"

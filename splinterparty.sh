#!/usr/bin/env sh
set -eu

# Splinterparty all-in-one installer and service manager
#
# Usage:
#   ./splinterparty.sh all              install, run setup if needed, install/start service
#   ./splinterparty.sh install          install dependencies, build release binary, install binary
#   ./splinterparty.sh setup            run interactive Splinterparty setup
#   ./splinterparty.sh service-install  install and start the systemd user service
#   ./splinterparty.sh service-remove   stop and remove the systemd user service
#   ./splinterparty.sh start            start the service
#   ./splinterparty.sh stop             stop the service
#   ./splinterparty.sh restart          restart the service
#   ./splinterparty.sh status           show service status and recent logs
#   ./splinterparty.sh logs             follow service logs
#   ./splinterparty.sh run              run Splinterparty directly in the foreground
#   ./splinterparty.sh help             show this help

SERVICE_NAME="splinterparty"
BIN_DIR="$HOME/.local/bin"
BINARY="$BIN_DIR/splinterparty"
WORK_DIR="$(pwd)"
SERVICE_DIR="$HOME/.config/systemd/user"
SERVICE_FILE="$SERVICE_DIR/$SERVICE_NAME.service"
RELEASE_BINARY="$WORK_DIR/target/release/splinterparty"
CONFIG_FILE="$WORK_DIR/splinterparty.conf"

is_root() {
    [ "$(id -u)" -eq 0 ]
}

refuse_root() {
    if is_root; then
        echo "Error: do not run this script with sudo/root." >&2
        echo "Splinterparty is installed as a user service for the current user." >&2
        exit 1
    fi
}

usage() {
    cat <<USAGE
Splinterparty all-in-one installer and service manager

Usage:
  $0 all              install, setup if needed, install/start service
  $0 install          install dependencies, build release binary, install binary
  $0 setup            run interactive Splinterparty setup
  $0 service-install  install and start the systemd user service
  $0 service-remove   stop and remove the systemd user service
  $0 start            start the service
  $0 stop             stop the service
  $0 restart          restart the service
  $0 status           show service status and recent logs
  $0 logs             follow service logs
  $0 run              run Splinterparty directly in the foreground
  $0 help             show this help

Do not run this script with sudo.
USAGE
}

check_home_writable() {
    if [ ! -w "$HOME" ]; then
        echo "Error: HOME directory '$HOME' is not writable by $(whoami)." >&2
        echo "Fix with: sudo chown -R $(whoami):$(whoami) $HOME" >&2
        exit 1
    fi
}

install_build_deps() {
    echo "Installing build dependencies if needed..."

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
        echo "No supported package manager found." >&2
        echo "Install curl, a C build toolchain, pkg-config, and Rust manually." >&2
    fi
}

install_rust_if_needed() {
    if ! command -v cargo >/dev/null 2>&1; then
        echo "Cargo not found. Installing Rust with rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        # shellcheck disable=SC1091
        . "$HOME/.cargo/env"
    fi
}

check_cargo_project() {
    if [ ! -f "$WORK_DIR/Cargo.toml" ]; then
        echo "Error: Cargo.toml not found in $WORK_DIR" >&2
        echo "Run this script from the Splinterparty repository directory." >&2
        exit 1
    fi
}

check_systemd() {
    if ! command -v systemctl >/dev/null 2>&1; then
        echo "Error: systemctl not found. Service management requires systemd." >&2
        exit 1
    fi
}

service_exists() {
    [ -f "$SERVICE_FILE" ]
}

service_is_active() {
    check_systemd
    systemctl --user is-active --quiet "$SERVICE_NAME" 2>/dev/null
}

stop_service_if_running_for_update() {
    if command -v systemctl >/dev/null 2>&1 && service_exists && service_is_active; then
        echo "Existing service detected. Stopping service for update..."
        systemctl --user stop "$SERVICE_NAME"
        SERVICE_WAS_RUNNING=1
    else
        SERVICE_WAS_RUNNING=0
    fi
}

restart_service_after_update_if_needed() {
    if [ "${SERVICE_WAS_RUNNING:-0}" = "1" ]; then
        echo "Starting service again..."
        systemctl --user start "$SERVICE_NAME"
    fi
}

cmd_install() {
    refuse_root
    check_home_writable
    check_cargo_project
    install_build_deps
    install_rust_if_needed

    echo "Building Splinterparty release binary..."
    cargo build --release

    if [ ! -f "$RELEASE_BINARY" ]; then
        echo "Error: release binary not found at $RELEASE_BINARY" >&2
        exit 1
    fi

    stop_service_if_running_for_update

    echo "Installing binary to $BINARY..."
    mkdir -p "$BIN_DIR"

    # Use a temporary file + mv. This avoids 'Text file busy' problems when possible.
    TMP_BINARY="$BIN_DIR/.splinterparty.tmp.$$"
    install -m 755 "$RELEASE_BINARY" "$TMP_BINARY"
    mv -f "$TMP_BINARY" "$BINARY"

    restart_service_after_update_if_needed

    if ! printf '%s' "$PATH" | grep -q "$(printf '%s' "$BIN_DIR" | sed 's/[.[\*^$()+?{}|]/\\&/g')"; then
        echo
        echo "Note: $BIN_DIR is not currently in PATH."
        echo "You can still run Splinterparty with: $BINARY"
    fi

    echo
    echo "Splinterparty installed to $BINARY."
}

cmd_setup() {
    refuse_root

    if [ ! -x "$BINARY" ]; then
        echo "Error: binary not found at $BINARY" >&2
        echo "Run: $0 install" >&2
        exit 1
    fi

    "$BINARY" setup
}

check_binary() {
    if [ ! -x "$BINARY" ]; then
        echo "Error: binary not found at $BINARY" >&2
        echo "Run: $0 install" >&2
        exit 1
    fi
}

check_config() {
    if [ ! -f "$CONFIG_FILE" ]; then
        echo "Error: splinterparty.conf not found in $WORK_DIR" >&2
        echo "Run setup first:" >&2
        echo "  $0 setup" >&2
        exit 1
    fi
}

enable_linger_best_effort() {
    if command -v loginctl >/dev/null 2>&1; then
        if loginctl show-user "$(whoami)" 2>/dev/null | grep -q '^Linger=yes'; then
            return 0
        fi

        echo "Enabling linger so the user service can start at boot..."
        if sudo loginctl enable-linger "$(whoami)" 2>/dev/null; then
            echo "Linger enabled."
        else
            echo "Warning: could not enable lingering automatically." >&2
            echo "Run manually if you want boot startup without login:" >&2
            echo "  sudo loginctl enable-linger $(whoami)" >&2
        fi
    else
        echo "Warning: loginctl not found. Cannot enable lingering automatically." >&2
    fi
}

cmd_service_install() {
    refuse_root
    check_systemd
    check_binary
    check_config

    mkdir -p "$SERVICE_DIR"

    cat > "$SERVICE_FILE" <<EOF_SERVICE
[Unit]
Description=Splinterparty fileserver
After=network.target

[Service]
Type=simple
WorkingDirectory=$WORK_DIR
ExecStart=$BINARY
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF_SERVICE

    enable_linger_best_effort

    systemctl --user daemon-reload
    systemctl --user enable "$SERVICE_NAME"
    systemctl --user restart "$SERVICE_NAME"

    echo "Service installed and started."
    echo "It will start automatically when your user service manager starts."
    echo
    systemctl --user status "$SERVICE_NAME" --no-pager || true
}

cmd_service_remove() {
    refuse_root
    check_systemd

    systemctl --user stop "$SERVICE_NAME" 2>/dev/null || true
    systemctl --user disable "$SERVICE_NAME" 2>/dev/null || true

    if [ -f "$SERVICE_FILE" ]; then
        rm -f "$SERVICE_FILE"
        systemctl --user daemon-reload
        echo "Service removed."
    else
        echo "No service file found, nothing to remove."
    fi
}

cmd_start() {
    refuse_root
    check_systemd
    systemctl --user start "$SERVICE_NAME"
    echo "Started."
}

cmd_stop() {
    refuse_root
    check_systemd
    systemctl --user stop "$SERVICE_NAME"
    echo "Stopped."
}

cmd_restart() {
    refuse_root
    check_systemd
    systemctl --user restart "$SERVICE_NAME"
    echo "Restarted."
}

cmd_status() {
    refuse_root
    check_systemd
    systemctl --user status "$SERVICE_NAME" --no-pager || true
    echo
    echo "Recent logs:"
    journalctl --user -u "$SERVICE_NAME" -n 30 --no-pager || true
}

cmd_logs() {
    refuse_root
    check_systemd
    journalctl --user -u "$SERVICE_NAME" -f
}

cmd_run() {
    refuse_root
    check_binary
    "$BINARY"
}

cmd_all() {
    refuse_root
    cmd_install

    if [ ! -f "$CONFIG_FILE" ]; then
        echo
        echo "No splinterparty.conf found. Starting setup..."
        cmd_setup
    else
        echo
        echo "Existing splinterparty.conf found. Skipping setup."
    fi

    cmd_service_install
}

case "${1:-help}" in
    all)             cmd_all ;;
    install)         cmd_install ;;
    setup)           cmd_setup ;;
    service-install) cmd_service_install ;;
    service-remove)  cmd_service_remove ;;
    remove)          cmd_service_remove ;;
    start)           cmd_start ;;
    stop)            cmd_stop ;;
    restart)         cmd_restart ;;
    status)          cmd_status ;;
    logs)            cmd_logs ;;
    run)             cmd_run ;;
    help|--help|-h)  usage ;;
    *)
        echo "Unknown command: $1" >&2
        echo >&2
        usage >&2
        exit 1
        ;;
esac

#!/usr/bin/env sh
set -eu

# Splinterparty systemd user service manager
# Usage:
#   ./service.sh install   - install and enable the user service
#   ./service.sh remove    - stop and remove the user service
#   ./service.sh start     - start the user service
#   ./service.sh stop      - stop the user service
#   ./service.sh status    - show service status and recent logs
#   ./service.sh logs      - follow live logs

if [ "$(id -u)" -eq 0 ]; then
    echo "Error: do not run this script with sudo/root." >&2
    echo "This installs a systemd user service for your normal user." >&2
    echo "Run: ./service.sh ${1:-install}" >&2
    exit 1
fi

SERVICE_NAME="splinterparty"
BINARY="$HOME/.local/bin/splinterparty"
WORK_DIR="$(pwd)"
SERVICE_DIR="$HOME/.config/systemd/user"
SERVICE_FILE="$SERVICE_DIR/$SERVICE_NAME.service"

check_systemd() {
    if ! command -v systemctl >/dev/null 2>&1; then
        echo "Error: systemctl not found. This script requires systemd." >&2
        exit 1
    fi
}

check_user_systemd() {
    if ! systemctl --user show-environment >/dev/null 2>&1; then
        echo "Error: systemd user session is not available." >&2
        echo "Try logging out/in, or run: loginctl enable-linger $(whoami)" >&2
        exit 1
    fi
}

check_binary() {
    if [ ! -x "$BINARY" ]; then
        echo "Error: binary not found or not executable at $BINARY" >&2
        echo "Run ./install.sh first, without sudo." >&2
        exit 1
    fi
}

check_config() {
    if [ ! -f "$WORK_DIR/splinterparty.conf" ]; then
        echo "Error: splinterparty.conf not found in $WORK_DIR" >&2
        echo "Run setup first:" >&2
        echo "  $BINARY setup" >&2
        exit 1
    fi
}

cmd_install() {
    check_systemd
    check_user_systemd
    check_binary
    check_config

    mkdir -p "$SERVICE_DIR"

    cat > "$SERVICE_FILE" << EOF_SERVICE
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

    # Enable lingering so the user service can start at boot without login.
    if command -v loginctl >/dev/null 2>&1; then
        loginctl enable-linger "$(whoami)" 2>/dev/null || \
            echo "Warning: could not enable lingering automatically. You may need: sudo loginctl enable-linger $(whoami)" >&2
    fi

    systemctl --user daemon-reload
    systemctl --user enable "$SERVICE_NAME"
    systemctl --user restart "$SERVICE_NAME"

    echo "Service installed and started."
    echo "It will start automatically when your user service manager starts."
    echo
    systemctl --user status "$SERVICE_NAME" --no-pager || true
}

cmd_remove() {
    check_systemd
    check_user_systemd

    systemctl --user stop "$SERVICE_NAME" 2>/dev/null || true
    systemctl --user disable "$SERVICE_NAME" 2>/dev/null || true

    if [ -f "$SERVICE_FILE" ]; then
        rm "$SERVICE_FILE"
        systemctl --user daemon-reload
        echo "Service removed."
    else
        echo "No service file found, nothing to remove."
    fi
}

cmd_start() {
    check_systemd
    check_user_systemd
    systemctl --user start "$SERVICE_NAME"
    echo "Started."
}

cmd_stop() {
    check_systemd
    check_user_systemd
    systemctl --user stop "$SERVICE_NAME"
    echo "Stopped."
}

cmd_status() {
    check_systemd
    check_user_systemd
    systemctl --user status "$SERVICE_NAME" --no-pager || true
    echo
    echo "Recent logs:"
    journalctl --user -u "$SERVICE_NAME" -n 20 --no-pager || true
}

cmd_logs() {
    check_systemd
    check_user_systemd
    journalctl --user -u "$SERVICE_NAME" -f
}

case "${1:-}" in
    install) cmd_install ;;
    remove)  cmd_remove ;;
    start)   cmd_start ;;
    stop)    cmd_stop ;;
    status)  cmd_status ;;
    logs)    cmd_logs ;;
    *)
        echo "Usage: $0 {install|remove|start|stop|status|logs}"
        exit 1
        ;;
esac

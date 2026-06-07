#!/usr/bin/env sh
set -eu

# Splinterparty installer + setup + user-service manager
# Usage:
#   ./splinterparty.sh install          Build and install binary to ~/.local/bin
#   ./splinterparty.sh setup            Run Splinterparty setup
#   ./splinterparty.sh service-install  Install, enable, and start user service
#   ./splinterparty.sh remove           Stop and remove user service
#   ./splinterparty.sh start            Start user service
#   ./splinterparty.sh stop             Stop user service
#   ./splinterparty.sh restart          Restart user service
#   ./splinterparty.sh status           Show service status and recent logs
#   ./splinterparty.sh logs             Follow live logs
#   ./splinterparty.sh all              install + setup + service-install

SERVICE_NAME="splinterparty"
INSTALL_DIR="$HOME/.local/bin"
BINARY="$INSTALL_DIR/splinterparty"
WORK_DIR="$(pwd)"
SERVICE_DIR="$HOME/.config/systemd/user"
SERVICE_FILE="$SERVICE_DIR/$SERVICE_NAME.service"

fail_if_root() {
    if [ "$(id -u)" -eq 0 ]; then
        echo "Error: do not run this script with sudo/root." >&2
        echo "Run it as your normal user, for example:" >&2
        echo "  ./splinterparty.sh $*" >&2
        exit 1
    fi
}

check_home_writable() {
    if [ ! -w "$HOME" ]; then
        echo "Error: HOME directory '$HOME' is not writable by $(whoami)." >&2
        echo "Fix with: sudo chown -R $(whoami):$(whoami) $HOME" >&2
        exit 1
    fi
}

install_deps() {
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
}

ensure_cargo() {
    if ! command -v cargo >/dev/null 2>&1; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        # shellcheck disable=SC1091
        . "$HOME/.cargo/env"
    fi
}

check_systemd() {
    if ! command -v systemctl >/dev/null 2>&1; then
        echo "Error: systemctl not found. This service manager requires systemd." >&2
        exit 1
    fi
}

check_binary() {
    if [ ! -x "$BINARY" ]; then
        echo "Error: binary not found or not executable at $BINARY" >&2
        echo "Run first:" >&2
        echo "  ./splinterparty.sh install" >&2
        exit 1
    fi
}

check_config() {
    if [ ! -f "$WORK_DIR/splinterparty.conf" ]; then
        echo "Error: splinterparty.conf not found in $WORK_DIR" >&2
        echo "Run first:" >&2
        echo "  ./splinterparty.sh setup" >&2
        exit 1
    fi
}

enable_linger() {
    if ! command -v loginctl >/dev/null 2>&1; then
        echo "Warning: loginctl not found. User service may not start after reboot." >&2
        return 0
    fi

    if loginctl show-user "$(whoami)" 2>/dev/null | grep -q '^Linger=yes'; then
        return 0
    fi

    if loginctl enable-linger "$(whoami)" 2>/dev/null; then
        return 0
    fi

    if command -v sudo >/dev/null 2>&1; then
        echo "Enabling lingering requires administrator permission. You may be asked for your password."
        if sudo loginctl enable-linger "$(whoami)"; then
            return 0
        fi
    fi

    echo "Warning: could not enable lingering automatically." >&2
    echo "Run this once to allow auto-start after reboot:" >&2
    echo "  sudo loginctl enable-linger $(whoami)" >&2
}

cmd_install() {
    fail_if_root install
    check_home_writable
    install_deps
    ensure_cargo

    cargo build --release

    SERVICE_WAS_RUNNING=0

    if command -v systemctl >/dev/null 2>&1; then
        if systemctl --user is-active --quiet "$SERVICE_NAME" 2>/dev/null; then
            echo "Running service detected. Stopping Splinterparty before replacing binary..."
            systemctl --user stop "$SERVICE_NAME"
            SERVICE_WAS_RUNNING=1
        fi
    fi

    mkdir -p "$INSTALL_DIR"

    install -m 755 "target/release/$SERVICE_NAME" "$BINARY"

    if [ "$SERVICE_WAS_RUNNING" -eq 1 ]; then
        echo "Restarting Splinterparty service..."
        systemctl --user start "$SERVICE_NAME"
    fi

    echo
    if ! printf '%s' ":$PATH:" | grep -q ":$INSTALL_DIR:"; then
        echo "Note: $INSTALL_DIR is not currently in PATH."
        echo "You can still run Splinterparty with: $BINARY"
        echo
    fi

    echo "Splinterparty installed to $BINARY."
}

cmd_setup() {
    fail_if_root setup
    check_binary
    "$BINARY" setup
}

cmd_service_install() {
    fail_if_root service-install
    check_systemd
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

    enable_linger

    systemctl --user daemon-reload
    systemctl --user enable "$SERVICE_NAME"
    systemctl --user restart "$SERVICE_NAME"

    echo "Service installed and started."
    echo "It will start automatically when your user service manager starts."
    echo
    systemctl --user status "$SERVICE_NAME" --no-pager || true
}

cmd_remove() {
    fail_if_root remove
    check_systemd

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
    fail_if_root start
    check_systemd
    systemctl --user start "$SERVICE_NAME"
    echo "Started."
}

cmd_stop() {
    fail_if_root stop
    check_systemd
    systemctl --user stop "$SERVICE_NAME"
    echo "Stopped."
}

cmd_restart() {
    fail_if_root restart
    check_systemd
    systemctl --user restart "$SERVICE_NAME"
    echo "Restarted."
}

cmd_status() {
    fail_if_root status
    check_systemd
    systemctl --user status "$SERVICE_NAME" --no-pager || true
    echo
    echo "Recent logs:"
    journalctl --user -u "$SERVICE_NAME" -n 20 --no-pager || true
}

cmd_logs() {
    fail_if_root logs
    check_systemd
    journalctl --user -u "$SERVICE_NAME" -f
}

cmd_tailscale() {
    fail_if_root tailscale

    echo "Enable Tailscale remote access? [y/N]"
    read -r answer

    case "$answer" in
        y|Y|yes|YES)
            ;;
        *)
            echo "Skipping Tailscale setup."
            return 0
            ;;
    esac

    if ! command -v tailscale >/dev/null 2>&1; then
        echo "Tailscale is not installed."
        echo "Installing Tailscale..."
        curl -fsSL https://tailscale.com/install.sh | sh
    fi

    echo "Starting tailscaled..."
    sudo systemctl enable --now tailscaled

    echo
    echo "Now logging into Tailscale."
    echo "A login URL may appear. Open it and sign in."
    sudo tailscale up

    TS_IP="$(tailscale ip -4 2>/dev/null | head -n 1 || true)"

    if [ -n "$TS_IP" ]; then
        echo
        echo "Tailscale remote access enabled:"
        echo "  http://$TS_IP:8080"
    else
        echo "Tailscale was started, but no Tailscale IP was found."
    fi
}

cmd_tailscale_status() {
    fail_if_root tailscale-status

    if ! command -v tailscale >/dev/null 2>&1; then
        echo "Tailscale is not installed."
        exit 1
    fi

    tailscale status || true

    TS_IP="$(tailscale ip -4 2>/dev/null | head -n 1 || true)"

    if [ -n "$TS_IP" ]; then
        echo
        echo "Splinterparty remote URL:"
        echo "  http://$TS_IP:8080"
    fi
}

cmd_all() {
    cmd_install
    cmd_setup
    cmd_tailscale
    cmd_service_install
}

cmd_update() {
    fail_if_root update

    if command -v git >/dev/null 2>&1 && [ -d .git ]; then
        git pull
    fi

    cmd_install
}

case "${1:-}" in
    install)         cmd_install ;;
    setup)           cmd_setup ;;
    service-install) cmd_service_install ;;
    remove)          cmd_remove ;;
    start)           cmd_start ;;
    stop)            cmd_stop ;;
    restart)         cmd_restart ;;
    status)          cmd_status ;;
    logs)            cmd_logs ;;
    tailscale)       cmd_tailscale ;;
    tailscale-status) cmd_tailscale_status ;;
    all)             cmd_all ;;
    update)          cmd_update ;;
    *)
        echo "Usage: $0 {install|setup|service-install|remove|start|stop|restart|status|logs|tailscale|tailscale-status|all|update}"
        exit 1
        ;;
esac

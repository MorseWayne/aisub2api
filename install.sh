#!/usr/bin/env bash
set -euo pipefail

REPO="MorseWayne/aisub2api"
INSTALL_DIR="${AISUB2API_INSTALL_DIR:-$HOME/.local/bin}"
SERVICE_BINARY=""
SERVICE_WAS_RUNNING=0

detect_platform() {
    local arch os
    arch="$(uname -m)"
    os="$(uname -s)"

    case "$os" in
        Linux)
            case "$arch" in
                x86_64)  echo "x86_64-unknown-linux-musl" ;;
                aarch64) echo "aarch64-unknown-linux-gnu" ;;
                *)       echo "Unsupported architecture: $arch" >&2; exit 1 ;;
            esac
            ;;
        Darwin)
            case "$arch" in
                x86_64)  echo "x86_64-apple-darwin" ;;
                arm64)   echo "aarch64-apple-darwin" ;;
                *)       echo "Unsupported architecture: $arch" >&2; exit 1 ;;
            esac
            ;;
        *)
            echo "Unsupported OS: $os. Download manually from GitHub Releases." >&2
            exit 1
            ;;
    esac
}

find_existing_binary() {
    if [[ -x "${INSTALL_DIR}/aisub2api" ]]; then
        echo "${INSTALL_DIR}/aisub2api"
    elif command -v aisub2api >/dev/null 2>&1; then
        command -v aisub2api
    elif [[ -x "${INSTALL_DIR}/claude-proxy" ]]; then
        echo "${INSTALL_DIR}/claude-proxy"
    elif command -v claude-proxy >/dev/null 2>&1; then
        command -v claude-proxy
    fi
}

daemon_is_running() {
    local binary="$1"
    local status

    if [[ -z "$binary" ]]; then
        return 1
    fi

    status="$("$binary" server status 2>/dev/null || true)"
    grep -Eq "(aisub2api|claude-proxy) is running" <<<"$status"
}

find_running_binary() {
    local candidate
    local candidates=(
        "${INSTALL_DIR}/aisub2api"
        "${INSTALL_DIR}/claude-proxy"
    )

    if command -v aisub2api >/dev/null 2>&1; then
        candidates+=("$(command -v aisub2api)")
    fi
    if command -v claude-proxy >/dev/null 2>&1; then
        candidates+=("$(command -v claude-proxy)")
    fi

    for candidate in "${candidates[@]}"; do
        if [[ -x "$candidate" ]] && daemon_is_running "$candidate"; then
            echo "$candidate"
            return 0
        fi
    done
    return 1
}

confirm_prompt() {
    local prompt="$1"
    local answer

    if [[ ! -r /dev/tty ]]; then
        echo "No interactive terminal is available for confirmation." >&2
        return 1
    fi

    printf "%s [y/N] " "$prompt" >/dev/tty
    IFS= read -r answer </dev/tty || return 1

    case "$answer" in
        y|Y|yes|YES|Yes) return 0 ;;
        *) return 1 ;;
    esac
}

prepare_existing_service() {
    SERVICE_BINARY="$(find_running_binary || find_existing_binary || true)"

    if daemon_is_running "$SERVICE_BINARY"; then
        SERVICE_WAS_RUNNING=1
        echo "A running aisub2api or legacy daemon was detected."
        echo "Continuing installation will stop it before replacing the binary."
    fi

    if ! confirm_prompt "Continue installing aisub2api?"; then
        echo "Installation cancelled."
        exit 1
    fi
}

stop_existing_service() {
    if [[ "$SERVICE_WAS_RUNNING" -ne 1 ]]; then
        return
    fi

    if daemon_is_running "$SERVICE_BINARY"; then
        echo "Stopping existing aisub2api daemon..."
        "$SERVICE_BINARY" server stop
    else
        echo "aisub2api daemon is no longer running."
    fi

    if daemon_is_running "$SERVICE_BINARY"; then
        echo "Failed to stop the existing aisub2api daemon." >&2
        exit 1
    fi
}

cleanup_legacy_install() {
    local legacy_binary="${INSTALL_DIR}/claude-proxy"

    if [[ -e "$legacy_binary" ]]; then
        rm -f -- "$legacy_binary"
        echo "Removed legacy binary ${legacy_binary}"
    fi

    # Trigger state migration during installation. If both state directories
    # exist, the new binary keeps aisub2api authoritative and archives the
    # legacy directory instead of deleting credentials or waiting for startup.
    "${INSTALL_DIR}/aisub2api" config path >/dev/null
}

start_installed_service() {
    if ! confirm_prompt "Start aisub2api now?"; then
        return
    fi

    if confirm_prompt "Run aisub2api in the background?"; then
        echo "Starting aisub2api daemon..."
        "${INSTALL_DIR}/aisub2api" server start --daemon
    else
        echo "Starting aisub2api in the foreground..."
        "${INSTALL_DIR}/aisub2api" server start
    fi
}

TMP_DIR=""
cleanup() {
    if [[ -n "${TMP_DIR:-}" ]]; then
        rm -rf "$TMP_DIR"
    fi
}
trap cleanup EXIT

main() {
    local platform
    platform="$(detect_platform)"
    local archive="aisub2api-${platform}.tar.gz"
    local url="https://github.com/${REPO}/releases/latest/download/${archive}"

    prepare_existing_service

    echo "Downloading aisub2api for ${platform}..."
    echo "  URL: ${url}"

    TMP_DIR="$(mktemp -d)"

    curl -fsSL "$url" -o "${TMP_DIR}/${archive}"
    tar xzf "${TMP_DIR}/${archive}" -C "$TMP_DIR"

    stop_existing_service

    mkdir -p "$INSTALL_DIR"
    mv "${TMP_DIR}/aisub2api" "${INSTALL_DIR}/aisub2api"
    chmod +x "${INSTALL_DIR}/aisub2api"
    cleanup_legacy_install

    echo ""
    echo "Installed to ${INSTALL_DIR}/aisub2api"
    echo ""

    # Check if INSTALL_DIR is in PATH
    if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
        echo "NOTE: ${INSTALL_DIR} is not in your PATH."
        echo "Add it with:"
        echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
        echo ""
    fi

    "${INSTALL_DIR}/aisub2api" --version

    start_installed_service
}

main "$@"

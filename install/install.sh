#!/usr/bin/env bash
# ToolsHub Universal Installer
# Supports: Linux (Arch, Debian/Ubuntu, Fedora, Alpine), macOS, Windows (WSL)
# Usage: curl -fsSL https://toolshub.dev/install.sh | bash

set -euo pipefail

# ── Constants ─────────────────────────────────────────────────────────────────

REPO="https://github.com/toolshub/toolshub"
BINARY_NAME="toolshub"
INSTALL_DIR="${TOOLSHUB_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${TOOLSHUB_VERSION:-latest}"
XDG_CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
XDG_DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"

CONFIG_DIR="$XDG_CONFIG_HOME/toolshub"
DATA_DIR="$XDG_DATA_HOME/toolshub"

# ── Colours ───────────────────────────────────────────────────────────────────

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

# ── Helpers ───────────────────────────────────────────────────────────────────

info()    { echo -e "${CYAN}  ℹ${RESET} $*"; }
success() { echo -e "${GREEN}  ✓${RESET} $*"; }
warning() { echo -e "${YELLOW}  ⚠${RESET} $*"; }
error()   { echo -e "${RED}  ✗ ERROR:${RESET} $*" >&2; }
fatal()   { error "$*"; exit 1; }
bold()    { echo -e "${BOLD}$*${RESET}"; }

require() {
    command -v "$1" >/dev/null 2>&1 || fatal "Required command not found: $1"
}

# ── Banner ────────────────────────────────────────────────────────────────────

banner() {
    echo ""
    echo -e "${PURPLE}${BOLD}"
    echo "  ████████╗ ██████╗  ██████╗ ██╗     ███████╗██╗  ██╗██╗   ██╗██████╗ "
    echo "     ██╔══╝██╔═══██╗██╔═══██╗██║     ██╔════╝██║  ██║██║   ██║██╔══██╗"
    echo "     ██║   ██║   ██║██║   ██║██║     ███████╗███████║██║   ██║██████╔╝"
    echo "     ██║   ██║   ██║██║   ██║██║     ╚════██║██╔══██║██║   ██║██╔══██╗"
    echo "     ██║   ╚██████╔╝╚██████╔╝███████╗███████║██║  ██║╚██████╔╝██████╔╝"
    echo "     ╚═╝    ╚═════╝  ╚═════╝ ╚══════╝╚══════╝╚═╝  ╚═╝ ╚═════╝ ╚═════╝ "
    echo ""
    echo -e "         ${CYAN}The Intelligent CLI Operating Companion${PURPLE}"
    echo ""
    echo -e "${RESET}"
}

# ── OS Detection ──────────────────────────────────────────────────────────────

detect_os() {
    OS=""
    DISTRO=""
    ARCH=$(uname -m)

    case "$(uname -s)" in
        Linux*)
            OS="linux"
            if [ -f /etc/os-release ]; then
                # shellcheck source=/dev/null
                . /etc/os-release
                DISTRO="${ID:-unknown}"
            fi
            ;;
        Darwin*)
            OS="macos"
            DISTRO="macos"
            ;;
        MINGW*|CYGWIN*|MSYS*)
            OS="windows"
            DISTRO="windows"
            ;;
        *)
            fatal "Unsupported operating system: $(uname -s)"
            ;;
    esac

    # Normalise architecture
    case "$ARCH" in
        x86_64|amd64)  ARCH="x86_64" ;;
        aarch64|arm64) ARCH="aarch64" ;;
        *)             fatal "Unsupported architecture: $ARCH" ;;
    esac

    export OS DISTRO ARCH
}

# ── Rust Target ───────────────────────────────────────────────────────────────

resolve_target() {
    case "${OS}-${ARCH}" in
        linux-x86_64)   TARGET="x86_64-unknown-linux-musl" ;;
        linux-aarch64)  TARGET="aarch64-unknown-linux-musl" ;;
        macos-x86_64)   TARGET="x86_64-apple-darwin" ;;
        macos-aarch64)  TARGET="aarch64-apple-darwin" ;;
        windows-x86_64) TARGET="x86_64-pc-windows-msvc" ;;
        *)              fatal "No prebuilt binary for ${OS}-${ARCH}" ;;
    esac
    export TARGET
}

# ── Dependency Check ──────────────────────────────────────────────────────────

check_dependencies() {
    require curl
    if ! command -v tar >/dev/null 2>&1 && ! command -v unzip >/dev/null 2>&1; then
        fatal "Either 'tar' or 'unzip' must be available."
    fi
}

# ── Version Resolution ────────────────────────────────────────────────────────

resolve_version() {
    if [ "$VERSION" = "latest" ]; then
        info "Resolving latest version..."
        VERSION=$(curl -fsSL "https://api.github.com/repos/toolshub/toolshub/releases/latest" \
            | grep '"tag_name"' \
            | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')
        [ -z "$VERSION" ] && fatal "Could not determine latest version."
    fi
    export VERSION
    info "Installing version: ${BOLD}${VERSION}${RESET}"
}

# ── Download ──────────────────────────────────────────────────────────────────

download_binary() {
    DOWNLOAD_URL="${REPO}/releases/download/${VERSION}/toolshub-${TARGET}.tar.gz"
    TMP_DIR=$(mktemp -d)
    TMP_ARCHIVE="${TMP_DIR}/toolshub.tar.gz"

    info "Downloading from: $DOWNLOAD_URL"
    curl -fsSL --progress-bar "$DOWNLOAD_URL" -o "$TMP_ARCHIVE" \
        || fatal "Download failed. Check your internet connection."

    info "Extracting..."
    tar -xzf "$TMP_ARCHIVE" -C "$TMP_DIR"

    EXTRACTED_BINARY="${TMP_DIR}/${BINARY_NAME}"
    [ -f "$EXTRACTED_BINARY" ] || fatal "Binary not found in archive."
    export TMP_DIR EXTRACTED_BINARY
}

# ── Install Binary ────────────────────────────────────────────────────────────

install_binary() {
    mkdir -p "$INSTALL_DIR"
    install -m 755 "$EXTRACTED_BINARY" "${INSTALL_DIR}/${BINARY_NAME}"
    success "Installed ${BINARY_NAME} to ${INSTALL_DIR}/${BINARY_NAME}"
    rm -rf "$TMP_DIR"
}

# ── Create Default Config ─────────────────────────────────────────────────────

create_default_config() {
    mkdir -p "$CONFIG_DIR" "$DATA_DIR"

    if [ ! -f "${CONFIG_DIR}/config.toml" ]; then
        cat > "${CONFIG_DIR}/config.toml" << 'EOF'
[general]
theme     = "dark"
language  = "en"
telemetry = false

[ai]
provider    = "ollama"
model       = "llama3.2"
api_key     = ""
endpoint    = "http://localhost:11434"
max_tokens  = 4096
temperature = 0.7

[memory]
enabled            = true
max_facts          = 10000
doc_cache_ttl_hours = 168

[packages]
confirm_before_install  = true
show_commands_before_run = true

[safety]
check_commands          = true
danger_level_threshold  = "medium"

[display]
icons           = true
color           = true
syntax_highlight = true
pager           = true
EOF
        success "Created default config at ${CONFIG_DIR}/config.toml"
    else
        info "Existing config preserved."
    fi
}

# ── Shell Integration ─────────────────────────────────────────────────────────

install_shell_integration() {
    # Detect shell config file
    SHELL_CONFIG=""
    CURRENT_SHELL=$(basename "${SHELL:-/bin/bash}")

    case "$CURRENT_SHELL" in
        bash) SHELL_CONFIG="${HOME}/.bashrc" ;;
        zsh)  SHELL_CONFIG="${ZDOTDIR:-$HOME}/.zshrc" ;;
        fish) SHELL_CONFIG="${XDG_CONFIG_HOME}/fish/config.fish" ;;
        *)    warning "Unknown shell: $CURRENT_SHELL — skipping shell integration." ; return ;;
    esac

    PATH_SNIPPET='export PATH="$HOME/.local/bin:$PATH"'
    if ! grep -qF '.local/bin' "$SHELL_CONFIG" 2>/dev/null; then
        echo "" >> "$SHELL_CONFIG"
        echo "# ToolsHub" >> "$SHELL_CONFIG"
        echo "$PATH_SNIPPET" >> "$SHELL_CONFIG"
        success "Added ~/.local/bin to PATH in ${SHELL_CONFIG}"
    else
        info "PATH already includes ~/.local/bin"
    fi
}

# ── Systemd Service (Linux) ───────────────────────────────────────────────────

install_systemd_service() {
    [ "$OS" != "linux" ] && return
    command -v systemctl >/dev/null 2>&1 || return

    SYSTEMD_USER_DIR="${HOME}/.config/systemd/user"
    mkdir -p "$SYSTEMD_USER_DIR"

    cat > "${SYSTEMD_USER_DIR}/toolshub.service" << EOF
[Unit]
Description=ToolsHub Background Daemon
After=default.target
Documentation=${REPO}

[Service]
Type=simple
ExecStart=${INSTALL_DIR}/toolshub daemon
Restart=on-failure
RestartSec=10
MemoryMax=100M
CPUWeight=10

[Install]
WantedBy=default.target
EOF

    systemctl --user daemon-reload 2>/dev/null || true
    success "Installed systemd user service: toolshub.service"
    info "Enable with: systemctl --user enable --now toolshub"
}

# ── Verify Installation ───────────────────────────────────────────────────────

verify_installation() {
    if "${INSTALL_DIR}/${BINARY_NAME}" --version >/dev/null 2>&1; then
        INSTALLED_VERSION=$("${INSTALL_DIR}/${BINARY_NAME}" --version 2>&1 | head -1)
        success "Verification passed: ${INSTALLED_VERSION}"
    else
        warning "Binary installed but could not execute. Check your PATH."
    fi
}

# ── Post-install Message ──────────────────────────────────────────────────────

print_next_steps() {
    echo ""
    bold "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    success "ToolsHub ${VERSION} installed successfully!"
    bold "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""
    echo -e "  Restart your shell or run: ${CYAN}source ~/.bashrc${RESET}"
    echo ""
    echo -e "  ${BOLD}Quick start:${RESET}"
    echo -e "    ${CYAN}toolshub env${RESET}              — show your system environment"
    echo -e "    ${CYAN}toolshub explain rsync${RESET}    — explain a command in depth"
    echo -e "    ${CYAN}toolshub ask 'how do I...'${RESET} — ask AI anything"
    echo -e "    ${CYAN}toolshub tools${RESET}            — list all installed tools"
    echo -e "    ${CYAN}toolshub install ripgrep${RESET}  — install any package"
    echo -e "    ${CYAN}toolshub doctor${RESET}           — diagnose your setup"
    echo ""
    echo -e "  ${BOLD}Config:${RESET} ${CONFIG_DIR}/config.toml"
    echo -e "  ${BOLD}Docs:${RESET}   https://docs.toolshub.dev"
    echo ""
}

# ── Main ──────────────────────────────────────────────────────────────────────

main() {
    banner
    detect_os
    resolve_target
    check_dependencies
    resolve_version
    download_binary
    install_binary
    create_default_config
    install_shell_integration
    install_systemd_service
    verify_installation
    print_next_steps
}

main "$@"

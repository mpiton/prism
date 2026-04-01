#!/usr/bin/env bash
set -euo pipefail

# PRism — Setup Script
# Checks prerequisites and installs missing dependencies.

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

ok()   { printf "${GREEN}[OK]${NC}    %s\n" "$1"; }
warn() { printf "${YELLOW}[MISS]${NC}  %s\n" "$1"; }
fail() { printf "${RED}[FAIL]${NC}  %s\n" "$1"; }

MISSING_CMDS=()
MISSING_PKGS=()

# ── Command checks ──────────────────────────────────────────────

check_cmd() {
    local cmd="$1" name="${2:-$1}" install_hint="${3:-}"
    if command -v "$cmd" &>/dev/null; then
        local version
        version=$("$cmd" --version 2>/dev/null | head -1)
        ok "$name — $version"
    else
        warn "$name not found${install_hint:+ ($install_hint)}"
        MISSING_CMDS+=("$cmd")
    fi
}

echo "=== PRism Prerequisites Check ==="
echo ""
echo "--- Toolchains ---"

check_cmd rustc  "Rust compiler" "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
check_cmd cargo  "Cargo"
check_cmd node   "Node.js (>=20)" "https://nodejs.org or nvm install 20"
check_cmd npm    "npm"

# ── Node.js version check ───────────────────────────────────────

if command -v node &>/dev/null; then
    NODE_MAJOR=$(node -v | sed 's/v//' | cut -d. -f1)
    if [ "$NODE_MAJOR" -lt 20 ]; then
        fail "Node.js >= 20 required, got $(node -v)"
        MISSING_CMDS+=("node>=20")
    fi
fi

# ── Rust MSRV check (1.85+) ─────────────────────────────────────

if command -v rustc &>/dev/null; then
    RUST_VER=$(rustc --version | awk 'match($0, /[0-9]+\.[0-9]+/) { print substr($0, RSTART, RLENGTH) }')
    RUST_MAJOR=$(echo "$RUST_VER" | cut -d. -f1)
    RUST_MINOR=$(echo "$RUST_VER" | cut -d. -f2)
    if [ "$RUST_MAJOR" -eq 1 ] && [ "$RUST_MINOR" -lt 85 ]; then
        fail "Rust >= 1.85 required (got $RUST_VER) — run: rustup update"
        MISSING_CMDS+=("rust>=1.85")
    fi
fi

# ── Tauri CLI ────────────────────────────────────────────────────

echo ""
echo "--- Tauri CLI ---"

if cargo tauri --version &>/dev/null 2>&1; then
    ok "tauri-cli — $(cargo tauri --version 2>/dev/null)"
elif npx tauri --version &>/dev/null 2>&1; then
    ok "tauri-cli (npx) — $(npx tauri --version 2>/dev/null)"
else
    warn "tauri-cli not found (cargo install tauri-cli or npm install -g @tauri-apps/cli)"
    MISSING_CMDS+=("tauri-cli")
fi

# ── System libraries (Linux only) ───────────────────────────────

echo ""
echo "--- System Libraries ---"

if [[ "$(uname)" == "Linux" ]]; then
    check_pkg() {
        local pkg="$1"
        if dpkg -s "$pkg" &>/dev/null; then
            ok "$pkg"
        else
            warn "$pkg"
            MISSING_PKGS+=("$pkg")
        fi
    }

    REQUIRED_PKGS=(
        libwebkit2gtk-4.1-dev
        libgtk-3-dev
        libayatana-appindicator3-dev
        librsvg2-dev
        libssl-dev
        patchelf
    )

    for pkg in "${REQUIRED_PKGS[@]}"; do
        check_pkg "$pkg"
    done
elif [[ "$(uname)" == "Darwin" ]]; then
    ok "macOS — system libs bundled via Xcode"
else
    warn "Windows — check https://v2.tauri.app/start/prerequisites/"
fi

# ── Summary & Auto-install ──────────────────────────────────────

echo ""
echo "=== Summary ==="

if [ ${#MISSING_CMDS[@]} -eq 0 ] && [ ${#MISSING_PKGS[@]} -eq 0 ]; then
    ok "All prerequisites met!"
    echo ""
    echo "Run the project:"
    echo "  npm install    # install JS dependencies"
    echo "  npm run tauri dev   # start dev server"
    exit 0
fi

if [ ${#MISSING_CMDS[@]} -gt 0 ]; then
    echo ""
    fail "Missing tools: ${MISSING_CMDS[*]}"
    echo "  Install manually — see hints above."
fi

if [ ${#MISSING_PKGS[@]} -gt 0 ]; then
    echo ""
    warn "Missing system packages: ${MISSING_PKGS[*]}"
    echo ""
    read -rp "Install missing packages with apt? [y/N] " answer
    if [[ "$answer" =~ ^[Yy]$ ]]; then
        sudo apt-get update
        sudo apt-get install -y "${MISSING_PKGS[@]}"
        ok "System packages installed!"
    else
        echo "  Manual install:"
        echo "  sudo apt-get install -y ${MISSING_PKGS[*]}"
    fi
fi

# ── Install tauri-cli if missing ─────────────────────────────────

if [[ " ${MISSING_CMDS[*]} " == *" tauri-cli "* ]]; then
    echo ""
    read -rp "Install tauri-cli via cargo? [y/N] " answer
    if [[ "$answer" =~ ^[Yy]$ ]]; then
        cargo install tauri-cli
        ok "tauri-cli installed!"
    fi
fi

# ── Install npm deps ────────────────────────────────────────────

if command -v npm &>/dev/null; then
    SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
    PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
    if [ ! -d "$PROJECT_DIR/node_modules" ]; then
        echo ""
        read -rp "Install npm dependencies? [y/N] " answer
        if [[ "$answer" =~ ^[Yy]$ ]]; then
            npm --prefix "$PROJECT_DIR" install
            ok "npm dependencies installed!"
        fi
    fi
fi

echo ""
echo "Once all prerequisites are met, run:"
echo "  npm run tauri dev"

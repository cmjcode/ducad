#!/bin/bash
# ==============================================================================
# DUCAD Apple Universal Publish Script (iPadOS + macOS)
# ==============================================================================
# Usage: ./publish_apple_all.sh [OPTIONS]
#
# Otomatis melakukan build & upload kedua platform Apple sekaligus ke
# App Store Connect / TestFlight (Universal Purchase):
#   1. iPadOS: Build aarch64-apple-ios -> DUCAD.ipa -> Upload
#   2. macOS:  Build App Sandbox -> DUCAD.pkg -> Upload / Notarize
# ==============================================================================

set -e

# --- 1. Determine Directories ---
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -f "$SCRIPT_DIR/Cargo.toml" ]; then
    EDITOR_DIR="$SCRIPT_DIR"
    ROOT_DIR="$(dirname "$SCRIPT_DIR")"
elif [ -d "$SCRIPT_DIR/ducad-editor" ]; then
    EDITOR_DIR="$SCRIPT_DIR/ducad-editor"
    ROOT_DIR="$SCRIPT_DIR"
else
    EDITOR_DIR="$PWD"
    ROOT_DIR="$PWD"
fi

# Load .env if present
if [ -f "$ROOT_DIR/.env" ]; then
    set -a
    source "$ROOT_DIR/.env"
    set +a
elif [ -f "$EDITOR_DIR/.env" ]; then
    set -a
    source "$EDITOR_DIR/.env"
    set +a
fi

cd "$EDITOR_DIR"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
BOLD='\033[1m'
NC='\033[0m'

print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

APP_NAME="DUCAD"
VERSION=$(grep '^version' Cargo.toml 2>/dev/null | head -n1 | cut -d '"' -f2 || echo "0.1.0")

show_help() {
    echo "Usage: ./publish_apple_all.sh [OPTIONS]"
    echo ""
    echo "Otomatis melakukan build & upload kedua platform Apple sekaligus ke"
    echo "App Store Connect / TestFlight (Universal Purchase):"
    echo "  1. iPadOS: Build aarch64-apple-ios -> DUCAD.ipa -> Upload"
    echo "  2. macOS:  Build App Sandbox -> DUCAD.pkg -> Upload"
    echo ""
    echo "Options:"
    echo "  --help, -h       - Tampilkan pesan bantuan ini"
    echo ""
}

for arg in "$@"; do
    case "$arg" in
        --help|-h)
            show_help
            exit 0
            ;;
    esac
done

if [ -z "$APPLE_ID" ] || [ -z "$APPLE_PASSWORD" ]; then
    print_error "Kredensial Apple (APPLE_ID / APPLE_PASSWORD) belum terisi di file .env."
    echo "Silakan atur variabel APPLE_ID dan APPLE_PASSWORD di file .env untuk mengaktifkan otomatisasi upload."
    exit 1
fi

# --- PHASE 1: iPadOS Publish ---
echo -e "${MAGENTA}${BOLD}📱 [1/2] Membangun & Mengunggah Versi iPadOS (.ipa)...${NC}"
echo "----------------------------------------------------------"
chmod +x ./build_ipad.sh 2>/dev/null || true
./build_ipad.sh publish || {
    print_error "Gagal dalam proses build / publish iPadOS."
    exit 1
}
print_success "Versi iPadOS berhasil diproses & diunggah!"
echo ""

# --- PHASE 2: macOS Publish ---
echo -e "${MAGENTA}${BOLD}💻 [2/2] Membangun & Mengunggah Versi macOS (.pkg)...${NC}"
echo "----------------------------------------------------------"
chmod +x ./build_macos.sh 2>/dev/null || true
./build_macos.sh macos-pkg || {
    print_error "Gagal dalam proses build / packaging macOS .pkg."
    exit 1
}
print_success "Versi macOS (.pkg) berhasil diproses!"
echo ""

# --- Upload macOS PKG jika belum terunggah ---
MACOS_PKG="dist/macos/$APP_NAME-$VERSION.pkg"
if [ -f "$MACOS_PKG" ] && command -v xcrun &>/dev/null; then
    print_status "Mengunggah macOS .pkg ke App Store Connect..."
    xcrun altool --upload-app \
        -f "$MACOS_PKG" \
        -t osx \
        -u "$APPLE_ID" \
        -p "$APPLE_PASSWORD" || {
            print_warning "Upload macOS altool selesai dengan peringatan. Jika perlu, file pkg juga siap di Transporter.app."
        }
fi

# --- SUMMARY ---
echo ""
echo -e "${GREEN}${BOLD}🎉 UNIVERSAL PUBLISH COMPLETED!${NC}"
echo "=========================================================="
echo "Semua paket binary untuk Universal Purchase telah selesai dibuat:"
echo "  📱 iPadOS: dist/ios/$APP_NAME-$VERSION.ipa"
echo "  💻 macOS:  dist/macos/$APP_NAME-$VERSION.pkg"
echo ""
echo "ℹ️  Kedua paket siap / telah terunggah ke App Store Connect di bawah halaman aplikasi yang sama."

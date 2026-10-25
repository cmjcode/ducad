#!/usr/bin/env bash
# ==============================================================================
# auto-update.sh - Periksa dan perbarui DUCAD dari GitHub Releases
# Target: https://github.com/cmjcode/ducad/releases/latest
#
# Pendamping updater di dalam aplikasi (crates/ducad-update) untuk dipakai dari
# terminal. Nama aset sama dengan yang dipilih updater:
#   macOS : DUCAD-<versi>[-macos-<arch>].dmg        (+ .sha256)
#   Linux : ducad-<versi>-linux-x86_64.tar.gz       (+ .sha256)
#   Windows: pakai updater di aplikasi atau installer .msi dari halaman rilis.
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

REPO="${DUCAD_UPDATE_REPO:-cmjcode/ducad}"
LATEST_URL="https://github.com/${REPO}/releases/latest"
UA="DUCAD-auto-update.sh"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_status() { echo -e "${BLUE}[INFO]${NC} $1"; }
print_success() { echo -e "${GREEN}[OK]${NC} $1"; }
print_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }
print_error() { echo -e "${RED}[ERROR]${NC} $1" >&2; }

usage() {
    cat <<EOF
Penggunaan: $0 [OPSI]

Opsi:
  --check, -c        Hanya periksa versi tanpa mengunduh
  --install, -y      Unduh dan pasang tanpa bertanya
  --app <path>       macOS: bundel yang diperbarui (default: /Applications/DUCAD.app,
                     lalu ~/Applications/DUCAD.app)
  --dir <folder>     Linux: folder instalasi berisi executable 'ducad'
                     (default: folder dari 'command -v ducad')
  --help, -h         Tampilkan bantuan ini

Env: DUCAD_UPDATE_REPO=pemilik/nama untuk repositori lain.
EOF
}

# ------------------------------------------------------------------------------
# 0. Argumen (diproses sebelum akses jaringan)
CHECK_ONLY=false
APPLY_INSTALL=false
APP_PATH=""
LINUX_DIR=""
while [ $# -gt 0 ]; do
    case "$1" in
        --check|-c) CHECK_ONLY=true ;;
        --install|-i|--yes|-y) APPLY_INSTALL=true ;;
        --app) APP_PATH="${2:?--app butuh path}"; shift ;;
        --dir) LINUX_DIR="${2:?--dir butuh folder}"; shift ;;
        --help|-h) usage; exit 0 ;;
        *) print_error "Argumen tidak dikenal: $1"; usage; exit 2 ;;
    esac
    shift
done

OS_TYPE="$(uname -s)"
ARCH_TYPE="$(uname -m)"
case "$ARCH_TYPE" in
    arm64|aarch64) ARCH_TYPE=arm64 ;;
    x86_64|amd64) ARCH_TYPE=x86_64 ;;
esac

for tool in curl tar; do
    command -v "$tool" >/dev/null 2>&1 || { print_error "'$tool' tidak ditemukan."; exit 1; }
done

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

# Bandingkan versi tanpa sort -V (sort bawaan macOS lama tidak punya -V).
# Mengembalikan 0 bila $1 > $2. Sufiks pra-rilis (-rc1) diabaikan.
version_gt() {
    local a b i x y
    IFS=. read -r -a a <<<"${1%%-*}"
    IFS=. read -r -a b <<<"${2%%-*}"
    for i in 0 1 2; do
        x="${a[$i]:-0}"; y="${b[$i]:-0}"
        x="${x//[!0-9]/}"; y="${y//[!0-9]/}"
        x=$((10#${x:-0})); y=$((10#${y:-0}))
        if [ "$x" -gt "$y" ]; then return 0; fi
        if [ "$x" -lt "$y" ]; then return 1; fi
    done
    return 1
}

# ------------------------------------------------------------------------------
# 1. Versi terpasang: aplikasi yang benar-benar terpasang lebih dulu, baru
#    berkas VERSION repo (untuk checkout pengembangan).
CURRENT_VERSION=""
if [ "$OS_TYPE" = "Darwin" ]; then
    if [ -z "$APP_PATH" ]; then
        for candidate in "/Applications/DUCAD.app" "$HOME/Applications/DUCAD.app"; do
            if [ -d "$candidate" ]; then APP_PATH="$candidate"; break; fi
        done
    fi
    if [ -n "$APP_PATH" ] && [ -f "$APP_PATH/Contents/Info.plist" ]; then
        CURRENT_VERSION=$(/usr/bin/defaults read "$APP_PATH/Contents/Info.plist" CFBundleShortVersionString 2>/dev/null || echo "")
    fi
elif [ "$OS_TYPE" = "Linux" ]; then
    if [ -z "$LINUX_DIR" ] && command -v ducad >/dev/null 2>&1; then
        LINUX_DIR="$(dirname "$(readlink -f "$(command -v ducad)")")"
    fi
    if [ -n "$LINUX_DIR" ] && [ -f "$LINUX_DIR/VERSION" ]; then
        CURRENT_VERSION="$(tr -d ' \r\n' < "$LINUX_DIR/VERSION")"
    fi
fi
if [ -z "$CURRENT_VERSION" ] && [ -f "$ROOT_DIR/VERSION" ]; then
    CURRENT_VERSION="$(tr -d ' \r\n' < "$ROOT_DIR/VERSION")"
    print_warning "Instalasi DUCAD tidak ditemukan; memakai VERSION repo (v${CURRENT_VERSION})."
fi
CURRENT_VERSION="${CURRENT_VERSION:-0.0.0}"
CURRENT_VERSION="${CURRENT_VERSION#v}"

# ------------------------------------------------------------------------------
# 2. Rilis terbaru: redirect releases/latest (tanpa rate limit API), lalu API.
print_status "Memeriksa rilis terbaru dari ${LATEST_URL}..."
REDIRECT_LOC=$(curl -sSI -A "$UA" --max-time 20 "$LATEST_URL" 2>/dev/null \
    | tr -d '\r' | awk 'tolower($1)=="location:" {print $2}' | tail -n 1 || true)
if [[ "$REDIRECT_LOC" != */releases/tag/* ]]; then
    print_warning "Redirect tidak terbaca, mencoba GitHub API..."
    REDIRECT_LOC=$(curl -s -A "$UA" --max-time 20 "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null \
        | grep '"html_url":' | grep '/releases/tag/' | head -n 1 | cut -d '"' -f 4 || true)
fi
if [[ "$REDIRECT_LOC" != */releases/tag/* ]]; then
    print_error "Gagal memeriksa rilis terbaru dari GitHub (belum ada rilis atau tidak ada koneksi)."
    exit 1
fi

LATEST_TAG="${REDIRECT_LOC##*/releases/tag/}"
LATEST_TAG="${LATEST_TAG%%[/?#]*}"
LATEST_VERSION="${LATEST_TAG#v}"

print_status "Versi lokal:   v${CURRENT_VERSION}"
print_status "Versi terbaru: v${LATEST_VERSION} (${REDIRECT_LOC})"

if ! version_gt "$LATEST_VERSION" "$CURRENT_VERSION"; then
    print_success "DUCAD sudah versi terbaru (v${CURRENT_VERSION})."
    exit 0
fi
print_success "Versi baru tersedia: v${LATEST_VERSION} (terpasang: v${CURRENT_VERSION})."
[ "$CHECK_ONLY" = true ] && exit 0

case "$OS_TYPE" in
    Darwin|Linux) ;;
    *)
        print_status "Di Windows, pakai menu Pengaturan → Periksa Pembaruan di aplikasi,"
        print_status "atau unduh installer .msi dari: ${REDIRECT_LOC}"
        exit 0
        ;;
esac

# ------------------------------------------------------------------------------
# 3. Daftar aset rilis (nama berkas sebenarnya, bukan tebakan)
ASSET_NAMES=$(curl -fsSL -A "$UA" --max-time 30 \
    "https://github.com/${REPO}/releases/expanded_assets/${LATEST_TAG}" 2>/dev/null \
    | grep -oE "/${REPO}/releases/download/${LATEST_TAG}/[^\"'< ]+" \
    | sed "s#.*/##" | sort -u || true)
if [ -z "$ASSET_NAMES" ]; then
    print_error "Daftar aset rilis ${LATEST_TAG} tidak terbaca. Unduh manual dari: ${REDIRECT_LOC}"
    exit 1
fi
has_asset() { grep -qxF "$1" <<<"$ASSET_NAMES"; }

PKG=""
if [ "$OS_TYPE" = "Darwin" ]; then
    if [ "$ARCH_TYPE" = arm64 ]; then ALIASES="arm64 aarch64"; else ALIASES="x86_64 x64 amd64"; fi
    for alias in $ALIASES universal; do
        if has_asset "DUCAD-${LATEST_VERSION}-macos-${alias}.dmg"; then PKG="DUCAD-${LATEST_VERSION}-macos-${alias}.dmg"; break; fi
        if has_asset "DUCAD-${LATEST_VERSION}-${alias}.dmg"; then PKG="DUCAD-${LATEST_VERSION}-${alias}.dmg"; break; fi
    done
    # DMG tanpa label arsitektur; arsitekturnya diperiksa lipo setelah di-mount.
    if [ -z "$PKG" ] && has_asset "DUCAD-${LATEST_VERSION}.dmg"; then PKG="DUCAD-${LATEST_VERSION}.dmg"; fi
else
    if [ "$ARCH_TYPE" != x86_64 ]; then
        print_error "Rilis Linux hanya tersedia untuk x86_64 (mesin ini: ${ARCH_TYPE})."
        exit 1
    fi
    if has_asset "ducad-${LATEST_VERSION}-linux-x86_64.tar.gz"; then PKG="ducad-${LATEST_VERSION}-linux-x86_64.tar.gz"; fi
fi
if [ -z "$PKG" ]; then
    print_error "Rilis ${LATEST_TAG} belum berisi paket untuk ${OS_TYPE}/${ARCH_TYPE}. Cek: ${REDIRECT_LOC}"
    exit 1
fi
if ! has_asset "${PKG}.sha256"; then
    print_error "Checksum ${PKG}.sha256 tidak ada di rilis; pembaruan dibatalkan demi keamanan."
    exit 1
fi

# Tujuan instalasi
if [ "$OS_TYPE" = "Darwin" ]; then
    APP_PATH="${APP_PATH:-/Applications/DUCAD.app}"
    DEST_PARENT="$(dirname "$APP_PATH")"
else
    if [ -z "$LINUX_DIR" ]; then
        print_error "Folder instalasi Linux tidak diketahui. Jalankan dengan --dir <folder berisi ducad>."
        exit 1
    fi
    DEST_PARENT="$LINUX_DIR"
fi
if [ ! -w "$DEST_PARENT" ]; then
    print_error "Tidak punya izin tulis di ${DEST_PARENT}. Jalankan ulang dengan izin yang sesuai."
    exit 1
fi

if [ "$APPLY_INSTALL" != true ]; then
    echo ""
    read -r -p "Unduh dan pasang DUCAD v${LATEST_VERSION} (${PKG}) sekarang? (y/N): " REPLY
    if [[ ! "$REPLY" =~ ^[Yy]$ ]]; then
        print_status "Pembaruan dibatalkan."
        exit 0
    fi
fi

if pgrep -x ducad >/dev/null 2>&1 || pgrep -x DUCAD >/dev/null 2>&1; then
    print_warning "DUCAD sedang berjalan; versi baru aktif setelah aplikasi dibuka ulang."
fi

# ------------------------------------------------------------------------------
# 4. Unduh + verifikasi SHA-256
TEMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/ducad-update-XXXXXX")
MOUNT_POINT=""
cleanup() {
    if [ -n "$MOUNT_POINT" ]; then hdiutil detach "$MOUNT_POINT" -quiet -force 2>/dev/null || true; fi
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

BASE_URL="https://github.com/${REPO}/releases/download/${LATEST_TAG}"
print_status "Mengunduh ${BASE_URL}/${PKG}..."
curl -fSL -A "$UA" --progress-bar "${BASE_URL}/${PKG}" -o "$TEMP_DIR/$PKG"
curl -fsSL -A "$UA" "${BASE_URL}/${PKG}.sha256" -o "$TEMP_DIR/$PKG.sha256"

EXPECTED=$(awk '{print tolower($1); exit}' "$TEMP_DIR/$PKG.sha256")
ACTUAL=$(sha256_of "$TEMP_DIR/$PKG" | tr '[:upper:]' '[:lower:]')
if [ "${#EXPECTED}" -ne 64 ] || [ "$EXPECTED" != "$ACTUAL" ]; then
    print_error "Checksum SHA-256 tidak cocok (harap: ${EXPECTED:-kosong}, dapat: ${ACTUAL}). Pembaruan dibatalkan."
    exit 1
fi
print_success "Checksum SHA-256 cocok."

# ------------------------------------------------------------------------------
# 5. Pasang. Versi baru disiapkan di sebelah yang lama lalu ditukar dengan
#    rename, supaya kegagalan di tengah jalan tidak menghapus instalasi lama.
if [ "$OS_TYPE" = "Darwin" ]; then
    MOUNT_POINT="$TEMP_DIR/mount"
    mkdir -p "$MOUNT_POINT"
    hdiutil attach "$TEMP_DIR/$PKG" -mountpoint "$MOUNT_POINT" -nobrowse -readonly -noautoopen -quiet
    NEW_APP=$(find "$MOUNT_POINT" -maxdepth 1 -name "*.app" -type d | head -n 1)
    if [ -z "$NEW_APP" ]; then
        print_error "DMG tidak berisi bundel .app."
        exit 1
    fi

    EXE_NAME=$(/usr/bin/defaults read "$NEW_APP/Contents/Info.plist" CFBundleExecutable 2>/dev/null || echo ducad)
    ARCHS=$(/usr/bin/lipo -archs "$NEW_APP/Contents/MacOS/$EXE_NAME" 2>/dev/null || echo "")
    if ! grep -qw "$ARCH_TYPE" <<<"$ARCHS"; then
        print_error "Bundel di ${PKG} untuk arsitektur '${ARCHS:-?}', bukan ${ARCH_TYPE}."
        exit 1
    fi

    APP_NAME="$(basename "$APP_PATH")"
    STAGED="$DEST_PARENT/.${APP_NAME}.ducad-new"
    BACKUP="$DEST_PARENT/.${APP_NAME}.ducad-old"
    rm -rf "$STAGED" "$BACKUP"
    /usr/bin/ditto "$NEW_APP" "$STAGED"
    # Bundel lama bertanda tangan Developer ID → bundel baru wajib valid juga.
    if /usr/bin/codesign -dv "$APP_PATH" 2>&1 | grep -q "^TeamIdentifier=[A-Z0-9]"; then
        if ! /usr/bin/codesign --verify --deep --strict "$STAGED" 2>/dev/null; then
            rm -rf "$STAGED"
            print_error "Tanda tangan kode bundel baru tidak valid. Pembaruan dibatalkan."
            exit 1
        fi
    fi
    if [ -d "$APP_PATH" ]; then mv "$APP_PATH" "$BACKUP"; fi
    if ! mv "$STAGED" "$APP_PATH"; then
        if [ -d "$BACKUP" ]; then mv "$BACKUP" "$APP_PATH"; fi
        print_error "Gagal menukar bundel; versi lama dipulihkan."
        exit 1
    fi
    rm -rf "$BACKUP"
    print_success "DUCAD berhasil diperbarui ke v${LATEST_VERSION} di ${APP_PATH}."
else
    tar -xzf "$TEMP_DIR/$PKG" -C "$TEMP_DIR"
    SRC_DIR="$TEMP_DIR/${PKG%.tar.gz}"
    [ -d "$SRC_DIR" ] || SRC_DIR="$TEMP_DIR"
    if [ ! -f "$SRC_DIR/ducad" ]; then
        print_error "Paket tidak berisi executable 'ducad'."
        exit 1
    fi
    # Per berkas: salin ke .nama.ducad-new lalu rename — aman walau ducad
    # sedang berjalan (proses lama tetap memegang berkas lamanya).
    for f in "$SRC_DIR"/*; do
        [ -f "$f" ] || continue
        name="$(basename "$f")"
        cp -p "$f" "$LINUX_DIR/.${name}.ducad-new"
        mv -f "$LINUX_DIR/.${name}.ducad-new" "$LINUX_DIR/$name"
    done
    print_success "DUCAD berhasil diperbarui ke v${LATEST_VERSION} di ${LINUX_DIR}."
fi

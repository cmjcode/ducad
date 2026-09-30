#!/bin/bash
# ==============================================================================
# clean_no_occt.sh — Bersihkan build cache Cargo tanpa menghapus OCCT
# DUCAD: CAD 2D/3D Kernel OpenCASCADE
# ==============================================================================
# OpenCASCADE (OCCT) membutuhkan waktu kompilasi 10–30 menit dari kode C++.
# Script ini membersihkan artefak build Rust/Cargo (debug, release, deps,
# incremental) dengan mengamankan direktori OCCT di target/.
#
# Penggunaan:
#   ./clean_no_occt.sh                # Bersihkan semua kecuali OCCT (Deep clean)
#   ./clean_no_occt.sh --workspace    # Bersihkan hanya crate workspace DUCAD
#   ./clean_no_occt.sh --debug        # Bersihkan hanya target/debug (simpan release & OCCT)
#   ./clean_no_occt.sh --release      # Bersihkan hanya target/release (simpan debug & OCCT)
#   ./clean_no_occt.sh -p <crate>     # Bersihkan crate tertentu (misal: -p ducad-ui)
#   ./clean_no_occt.sh --dry-run      # Simulasi / lihat apa yang akan dihapus
# ==============================================================================

set -euo pipefail

# --- Pewarnaan Terminal ---
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

print_info()    { echo -e "${BLUE}[INFO]${NC} $1"; }
print_success() { echo -e "${GREEN}[SUKSES]${NC} $1"; }
print_warning() { echo -e "${YELLOW}[PERINGATAN]${NC} $1"; }
print_error()   { echo -e "${RED}[ERROR]${NC} $1"; }
print_guard()   { echo -e "${CYAN}[SAFEGUARD]${NC} $1"; }

# --- Deteksi Direktori Kerja ---
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

TARGET_DIR="${CARGO_TARGET_DIR:-$EDITOR_DIR/target}"

# --- Bantuan Penggunaan ---
show_help() {
    echo -e "${BOLD}🛠️  DUCAD Cargo Clean Helper (Preserve OCCT)${NC}"
    echo -e "Membersihkan artefak build Cargo tanpa memicu kompilasi ulang OpenCASCADE C++ kernel."
    echo ""
    echo -e "${BOLD}PENGGUNAAN:${NC}"
    echo -e "  $(basename "$0") [OPSI]"
    echo ""
    echo -e "${BOLD}OPSI:${NC}"
    echo -e "  -a, --all            (Default) Deep clean: bersihkan SEMUA artefak (deps, debug,"
    echo -e "                       release, incremental) KECUALI direktori OCCT."
    echo -e "  -w, --workspace      Bersihkan hanya crate lokal DUCAD (ducad-app, ducad-ui,"
    echo -e "                       ducad-engine, dll.). Crate dependensi & OCCT tetap utuh."
    echo -e "      --debug          Bersihkan hanya target/debug (release & OCCT tetap ada)."
    echo -e "      --release        Bersihkan hanya target/release (debug & OCCT tetap ada)."
    echo -e "  -p, --package <NAMA> Bersihkan hanya crate spesifik (contoh: -p ducad-ui)."
    echo -e "  -n, --dry-run        Tampilkan informasi apa yang akan dibersihkan tanpa menghapus."
    echo -e "  -h, --help           Tampilkan panduan ini."
    echo ""
    echo -e "${BOLD}CONTOH:${NC}"
    echo -e "  $(basename "$0")                    # Bersihkan penuh target/ kecuali OCCT"
    echo -e "  $(basename "$0") --workspace        # Bersihkan cepat kode DUCAD saja (~1 detik)"
    echo -e "  $(basename "$0") -p ducad-app       # Bersihkan hanya build ducad-app"
    echo -e "  $(basename "$0") --dry-run          # Cek ukuran target dan direktori OCCT"
    echo ""
}

# --- Parse Argumen ---
MODE="all"
DRY_RUN=false
TARGET_PACKAGE=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        -a|--all)
            MODE="all"
            shift
            ;;
        -w|--workspace)
            MODE="workspace"
            shift
            ;;
        --debug)
            MODE="debug"
            shift
            ;;
        --release)
            MODE="release"
            shift
            ;;
        -p|--package)
            MODE="package"
            if [[ $# -lt 2 || "$2" =~ ^- ]]; then
                print_error "Opsi -p / --package membutuhkan nama crate (misal: -p ducad-ui)."
                exit 1
            fi
            TARGET_PACKAGE="$2"
            shift 2
            ;;
        -n|--dry-run)
            DRY_RUN=true
            shift
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        *)
            print_error "Argumen tidak dikenali: $1"
            echo "Jalankan '$(basename "$0") --help' untuk panduan penggunaan."
            exit 1
            ;;
    esac
done

# --- Validasi Direktori Target ---
if [ ! -d "$TARGET_DIR" ]; then
    print_info "Direktori target tidak ditemukan ($TARGET_DIR)."
    print_info "Tidak ada cache yang perlu dibersihkan."
    exit 0
fi

# Dapatkan ukuran total target saat ini
get_dir_size() {
    local dir="$1"
    if [ -d "$dir" ]; then
        du -sh "$dir" 2>/dev/null | cut -f1 || echo "0B"
    else
        echo "0B"
    fi
}

INITIAL_TARGET_SIZE=$(get_dir_size "$TARGET_DIR")

# --- Mode: Package ---
if [ "$MODE" = "package" ]; then
    print_info "Menjalankan cargo clean untuk paket: ${BOLD}$TARGET_PACKAGE${NC}..."
    if [ "$DRY_RUN" = true ]; then
        cargo clean --manifest-path "$EDITOR_DIR/Cargo.toml" -p "$TARGET_PACKAGE" --dry-run
    else
        cargo clean --manifest-path "$EDITOR_DIR/Cargo.toml" -p "$TARGET_PACKAGE"
        print_success "Paket $TARGET_PACKAGE berhasil dibersihkan!"
    fi
    exit 0
fi

# --- Mode: Workspace ---
if [ "$MODE" = "workspace" ]; then
    print_info "Menjalankan cargo clean untuk seluruh workspace DUCAD..."
    print_info "(Dependensi eksternal dan OCCT tidak disentuh)"
    if [ "$DRY_RUN" = true ]; then
        cargo clean --manifest-path "$EDITOR_DIR/Cargo.toml" --workspace --dry-run
    else
        cargo clean --manifest-path "$EDITOR_DIR/Cargo.toml" --workspace
        print_success "Workspace DUCAD berhasil dibersihkan!"
    fi
    exit 0
fi

# --- Mode: Debug Only ---
if [ "$MODE" = "debug" ]; then
    DEBUG_DIR="$TARGET_DIR/debug"
    if [ ! -d "$DEBUG_DIR" ]; then
        print_info "Direktori target/debug tidak ditemukan. Tidak ada yang perlu dibersihkan."
        exit 0
    fi
    DEBUG_SIZE=$(get_dir_size "$DEBUG_DIR")
    print_info "Target debug ditemukan: $DEBUG_SIZE"
    if [ "$DRY_RUN" = true ]; then
        print_info "[DRY-RUN] Akan menghapus: $DEBUG_DIR ($DEBUG_SIZE)"
        print_guard "OCCT di $TARGET_DIR/OCCT aman tidak disentuh."
    else
        print_info "Menghapus $DEBUG_DIR ($DEBUG_SIZE)..."
        rm -rf "$DEBUG_DIR"
        print_success "target/debug berhasil dibersihkan! ($DEBUG_SIZE dibebaskan)"
        print_guard "OCCT kernel tetap aman dan utuh."
    fi
    exit 0
fi

# --- Mode: Release Only ---
if [ "$MODE" = "release" ]; then
    RELEASE_DIR="$TARGET_DIR/release"
    if [ ! -d "$RELEASE_DIR" ]; then
        print_info "Direktori target/release tidak ditemukan. Tidak ada yang perlu dibersihkan."
        exit 0
    fi
    RELEASE_SIZE=$(get_dir_size "$RELEASE_DIR")
    print_info "Target release ditemukan: $RELEASE_SIZE"
    if [ "$DRY_RUN" = true ]; then
        print_info "[DRY-RUN] Akan menghapus: $RELEASE_DIR ($RELEASE_SIZE)"
        print_guard "OCCT di $TARGET_DIR/OCCT aman tidak disentuh."
    else
        print_info "Menghapus $RELEASE_DIR ($RELEASE_SIZE)..."
        rm -rf "$RELEASE_DIR"
        print_success "target/release berhasil dibersihkan! ($RELEASE_SIZE dibebaskan)"
        print_guard "OCCT kernel tetap aman dan utuh."
    fi
    exit 0
fi

# --- Mode: Deep Clean (All except OCCT) ---
# 1. Deteksi semua direktori OCCT di dalam target
occt_dirs=()
while IFS= read -r dir; do
    if [ -n "$dir" ] && [ -d "$dir" ]; then
        occt_dirs+=("$dir")
    fi
done < <(find "$TARGET_DIR" -mindepth 1 -maxdepth 3 -type d -name "OCCT" -prune 2>/dev/null || true)

echo ""
echo -e "${BOLD}================================================================${NC}"
echo -e "${BOLD}       DUCAD Deep Clean (Preserve OpenCASCADE Kernel)           ${NC}"
echo -e "${BOLD}================================================================${NC}"
print_info "Lokasi target       : $TARGET_DIR"
print_info "Ukuran target total : $INITIAL_TARGET_SIZE"

if [ ${#occt_dirs[@]} -eq 0 ]; then
    print_warning "Tidak ditemukan direktori OCCT di dalam $TARGET_DIR."
    print_warning "Menjalankan standard cargo clean..."
    if [ "$DRY_RUN" = true ]; then
        cargo clean --manifest-path "$EDITOR_DIR/Cargo.toml" --dry-run
    else
        cargo clean --manifest-path "$EDITOR_DIR/Cargo.toml"
        print_success "Standard cargo clean selesai."
    fi
    exit 0
fi

echo ""
print_guard "Ditemukan ${#occt_dirs[@]} direktori OCCT yang AKAN DILINDUNGI:"
TOTAL_OCCT_BYTES=0
for d in "${occt_dirs[@]}"; do
    size_str=$(get_dir_size "$d")
    rel_path="${d#$TARGET_DIR/}"
    echo -e "   🛡️  ${BOLD}$rel_path${NC} ($size_str)"
done

if [ "$DRY_RUN" = true ]; then
    echo ""
    print_info "[DRY-RUN] Ringkasan yang akan dieksekusi:"
    print_info "  - Melindungi direktori OCCT di atas secara atomik (mv)."
    print_info "  - Menghapus semua file dan folder lain di $TARGET_DIR."
    print_info "  - Mengembalikan direktori OCCT ke lokasi aslinya."
    print_info "  - Memulihkan file CACHEDIR.TAG."
    echo ""
    print_success "Simulasi selesai. Tidak ada berkas yang dihapus."
    exit 0
fi

# Siapkan direktori backup sementara pada filesystem/volume yang sama
# Menggunakan 'mv' di filesystem yang sama berlangsung instan (< 10ms)
BACKUP_PARENT="$(dirname "$TARGET_DIR")"
BACKUP_DIR="$BACKUP_PARENT/.occt_clean_backup_$$"

# Setup trap pengaman: jika script diinterupsi (Ctrl+C atau error),
# kembalikan OCCT ke posisi aslinya agar tidak hilang
cleanup_trap() {
    local exit_code=$?
    if [ -d "$BACKUP_DIR" ]; then
        echo ""
        print_warning "Proses terhenti! Mengembalikan direktori OCCT dari backup sementara..."
        mkdir -p "$TARGET_DIR"
        for d in "${occt_dirs[@]}"; do
            rel_path="${d#$TARGET_DIR/}"
            if [ -d "$BACKUP_DIR/$rel_path" ]; then
                mkdir -p "$TARGET_DIR/$(dirname "$rel_path")"
                mv "$BACKUP_DIR/$rel_path" "$TARGET_DIR/$rel_path" 2>/dev/null || true
            fi
        done
        rm -rf "$BACKUP_DIR" 2>/dev/null || true
        print_warning "OCCT telah dipulihkan ke $TARGET_DIR."
    fi
    exit "$exit_code"
}
trap cleanup_trap EXIT INT TERM

echo ""
print_info "1/4. Memindahkan direktori OCCT ke ruang aman sementara (instant)..."
mkdir -p "$BACKUP_DIR"
for d in "${occt_dirs[@]}"; do
    rel_path="${d#$TARGET_DIR/}"
    mkdir -p "$BACKUP_DIR/$(dirname "$rel_path")"
    mv "$d" "$BACKUP_DIR/$rel_path"
done

print_info "2/4. Membersihkan sisa target/ (debug, release, deps, incremental)..."
# Hapus isi target
rm -rf "$TARGET_DIR"
mkdir -p "$TARGET_DIR"

print_info "3/4. Mengembalikan direktori OCCT ke lokasi semula..."
for d in "${occt_dirs[@]}"; do
    rel_path="${d#$TARGET_DIR/}"
    mkdir -p "$TARGET_DIR/$(dirname "$rel_path")"
    mv "$BACKUP_DIR/$rel_path" "$TARGET_DIR/$rel_path"
done

print_info "4/4. Menata ulang CACHEDIR.TAG..."
cat << 'EOF' > "$TARGET_DIR/CACHEDIR.TAG"
Signature: 8a477f597d28d1727207f7123bece77e
# This file is a cache directory tag created by cargo.
# For information about cache directory tags, see:
#   https://bford.info/cachedir/spec.html
EOF

# Bersihkan direktori backup dan lepas trap
rm -rf "$BACKUP_DIR"
trap - EXIT INT TERM

FINAL_TARGET_SIZE=$(get_dir_size "$TARGET_DIR")

echo ""
print_success "Pembersihan selesai tanpa menyentuh kernel OCCT! 🎉"
echo -e "   • Ukuran target sebelum : ${BOLD}$INITIAL_TARGET_SIZE${NC}"
echo -e "   • Ukuran target sesudah : ${BOLD}$FINAL_TARGET_SIZE${NC} (berisi OCCT)"
echo ""
print_info "Build berikutnya TIDAK AKAN mengompilasi ulang OpenCASCADE (hemat 10–30 menit)."
print_info "Coba jalankan: ${BOLD}cargo build -p ducad-app${NC}"
echo ""

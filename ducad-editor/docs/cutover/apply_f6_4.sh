#!/usr/bin/env bash
# PLAN_MIGRASI_TRUCK F6.4 — cutover: hapus backend OpenCASCADE sepenuhnya.
#
# JANGAN dijalankan sebelum soak 0.6.0-beta selesai (2 minggu tanpa crash
# kernel di jurnal panik) DAN R7 (valve_body) tertangani — lihat status F6 di
# docs/PLAN_MIGRASI_TRUCK.md.
#
# Isi cutover (diverifikasi di worktree terpisah, 2026-10-19):
#   - F6.4-delete.txt   : path yang dihapus (backend/occt, toolchain cmake
#                         iOS/Android, clean_no_occt.sh ×3, kernel_select.rs,
#                         build.rs kernel, submodule opencascade-rs, .gitmodules)
#   - F6.4-modify.patch : sisanya (manifest tanpa fitur backend, kernel tanpa
#                         cabang OCCT, skrip build/CI/rilis tanpa cmake &
#                         submodule, Cargo.lock, AUR, dokumen, VERSION 0.6.0)
#
# Pemakaian (dari mana saja di dalam repo, working tree bersih):
#   ducad-editor/docs/cutover/apply_f6_4.sh [--check]
# --check hanya menguji apakah patch masih bisa diterapkan.
# Hasilnya TIDAK di-commit; tinjau `git status`, jalankan gate F6, lalu commit.
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
DIR="ducad-editor/docs/cutover"
PATCH="$DIR/F6.4-modify.patch"
DELETE="$DIR/F6.4-delete.txt"

if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "working tree/index harus bersih (commit atau simpan perubahan dulu)" >&2
  exit 1
fi

git apply --check "$PATCH"
if [ "${1:-}" = "--check" ]; then
  echo "patch masih bisa diterapkan bersih"
  exit 0
fi

# Submodule: lepas dari working tree dulu supaya `git rm` tidak menolak.
if [ -e ducad-editor/vendors/opencascade-rs/.git ]; then
  git submodule deinit -f ducad-editor/vendors/opencascade-rs
fi
grep -v '^\s*#' "$DELETE" | grep -v '^\s*$' | while read -r path; do
  git rm -r -q --ignore-unmatch -- "$path"
done
git apply --index "$PATCH"

cat <<'MSG'
Cutover F6.4 diterapkan (belum di-commit). Berikutnya:
  cd ducad-editor
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  cargo check -p ducad-kernel --target wasm32-unknown-unknown
  git grep -n -i "occt\|opencascade" -- . ':!ducad-editor/vendors'   # sisa = riwayat/dokumen lama
Lalu commit, dan rilis 0.6.0 (F6.5): scripts/release-tag.sh --no-push
(build number default 0.6.0 = 60 sama dengan beta — set BUILD=61).
Sisa git internal submodule (opsional): rm -rf .git/modules/ducad-editor/vendors/opencascade-rs
MSG

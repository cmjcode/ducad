#!/usr/bin/env bash
set -uo pipefail

# Gerbang rilis workflow designer (Step 5 docs/PLAN_DESIGNER_UX.md).
#
# Pemakaian: scripts/release-gate.sh
#
# Menjalankan tes otomatis inti per metrik kualitas rilis, mencetak tabel
# ringkasan (juga ke $GITHUB_STEP_SUMMARY bila ada), dan keluar dengan kode
# bukan nol bila ada yang gagal. Dipakai lane CI "designer" (memblokir) dan
# dijalankan manual sebelum scripts/release-tag.sh. Bagian GUI yang tidak
# bisa diotomatiskan ada di ducad-editor/docs/CEKLIS_UJI_GUI.md Bagian P;
# lihat ducad-editor/docs/GERBANG_RILIS.md.

case "${1:-}" in
  -h|--help) sed -n '4,13p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
  "") ;;
  *) echo "argumen tidak dikenal: $1" >&2; exit 2 ;;
esac

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT/ducad-editor"

# "metrik|argumen setelah `cargo test`". Baris dengan metrik yang sama harus
# berurutan (dijumlahkan per metrik; kompatibel bash 3.2 macOS).
GATE_GROUPS=(
  "Workflow proyek contoh (oplog)|-p ducad-engine --test sample_project"
  "Workflow proyek contoh (oplog)|-p ducad-engine --lib samples"
  "Regresi simpan/buka|-p ducad-engine --test session_file"
  "Regresi simpan/buka|-p ducad-engine --test format_validation native_"
  "Regresi simpan/buka|-p ducad-engine --test format_validation corrupt_"
  "Regresi simpan/buka|-p ducad-io --test native_format"
  "Kegagalan ekspor|-p ducad-engine --test format_validation step_"
  "Kegagalan ekspor|-p ducad-engine --test format_validation stl_"
  "Kegagalan ekspor|-p ducad-engine --test format_validation drawing_"
  "Kegagalan ekspor|-p ducad-io --lib dxf"
  "Crash / error fatal|-p ducad-engine --test kernel_budget"
  "Crash / error fatal|-p ducad-engine --test hostile"
  "Tutorial dan saran GUI|-p ducad-app --lib onboarding_ui"
  "Tutorial dan saran GUI|-p ducad-app --lib next_action"
  "Tutorial dan saran GUI|-p ducad-app --lib step_export_uses_visible"
  "Tutorial dan saran GUI|-p ducad-ui --test onboarding_render"
  "Teks UI (glyph, terjemahan)|-p ducad-ui --test ui_glyph_coverage"
  "Teks UI (glyph, terjemahan)|-p ducad-i18n"
)

log="$(mktemp)"
trap 'rm -f "$log"' EXIT

table="| Metrik | Lulus | Gagal | Status |\n|---|---:|---:|---|\n"
total_bad=0
metric=""
m_pass=0
m_bad=0

flush() {
  [[ -z "$metric" ]] && return
  local status="lulus"
  (( m_bad > 0 )) && status="GAGAL"
  table+="| $metric | $m_pass | $m_bad | $status |\n"
  total_bad=$(( total_bad + m_bad ))
}

for entry in "${GATE_GROUPS[@]}"; do
  name="${entry%%|*}"
  args="${entry#*|}"
  if [[ "$name" != "$metric" ]]; then
    flush
    metric="$name"; m_pass=0; m_bad=0
  fi
  echo "==> cargo test $args"
  # shellcheck disable=SC2086
  cargo test $args >"$log" 2>&1
  code=$?
  p=$(grep -E '^test result:' "$log" | awk '{s+=$4} END {print s+0}')
  f=$(grep -E '^test result:' "$log" | awk '{s+=$6} END {print s+0}')
  m_pass=$(( m_pass + p ))
  m_bad=$(( m_bad + f ))
  if (( code != 0 )); then
    grep -E '^test .* FAILED$|^error(\[|:)|panicked at' "$log" | head -20
    # Gagal kompilasi atau crash di luar tes tetap dihitung gagal.
    (( f == 0 )) && m_bad=$(( m_bad + 1 ))
  elif (( p == 0 )); then
    echo "   tidak ada tes yang cocok untuk '$args' (nama tes berubah?)" >&2
    m_bad=$(( m_bad + 1 ))
  fi
done
flush

summary="## Gerbang rilis workflow designer\n\n$table\nChecklist GUI manual: docs/CEKLIS_UJI_GUI.md Bagian P (wajib sebelum rilis).\n"
echo
printf '%b' "$summary"
if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  printf '%b' "$summary" >> "$GITHUB_STEP_SUMMARY"
fi

(( total_bad == 0 ))

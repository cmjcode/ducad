#!/usr/bin/env bash
set -euo pipefail

# Perbarui metadata paket AUR DUCAD dan dorong ke repo AUR eksternal.
#
# Pemakaian: scripts/update-aur.sh [ducad] [--no-push] [--skip-tag-check]
#
# Langkah:
#   1. Baca versi dari VERSION (root repo) dan pastikan sama dengan
#      `[workspace.package].version` di ducad-editor/Cargo.toml.
#   2. Pastikan tag `v<versi>` sudah ada di remote `origin` — PKGBUILD
#      membangun dari tag itu, jadi tanpa tag paket AUR pasti gagal.
#   3. Set pkgver/pkgrel di aur/<paket>/PKGBUILD, hitung ulang sha256
#      berkas sumber lokal (mis. ducad.desktop).
#   4. Buat .SRCINFO: pakai `makepkg --printsrcinfo` bila ada, kalau tidak
#      lewat container archlinux (docker/podman), dan terakhir emitter bash
#      bawaan (cukup untuk PKGBUILD paket tunggal seperti ini).
#   5. Salin PKGBUILD, .SRCINFO, dan berkas sumber lokal ke repo AUR
#      (clone otomatis dari aur.archlinux.org bila belum ada), lalu commit
#      dan push ke cabang `master`.
#
# Variabel lingkungan:
#   TARGET_REPO   lokasi clone repo AUR (bawaan: <induk repo>/<paket>-aur)
#   AUR_SSH_URL   URL clone AUR (bawaan: ssh://aur@aur.archlinux.org/<paket>.git)

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EDITOR_DIR="${ROOT_DIR}/ducad-editor"

PACKAGE="ducad"
PUSH=1
TAG_CHECK=1
for arg in "$@"; do
  case "${arg}" in
    --no-push) PUSH=0 ;;
    --skip-tag-check) TAG_CHECK=0 ;;
    -h|--help)
      sed -n '3,25p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    -*)
      echo "Opsi tidak dikenal: ${arg}" >&2
      exit 1
      ;;
    *) PACKAGE="${arg}" ;;
  esac
done

case "${PACKAGE}" in
  ducad) ;;
  *)
    echo "Varian paket tidak dikenal: ${PACKAGE}" >&2
    echo "Pemakaian: $0 [ducad] [--no-push] [--skip-tag-check]" >&2
    exit 1
    ;;
esac

PKGBUILD_DIR="${ROOT_DIR}/aur/${PACKAGE}"
PKGBUILD="${PKGBUILD_DIR}/PKGBUILD"
DEFAULT_TARGET_REPO="$(cd "${ROOT_DIR}/.." && pwd)/${PACKAGE}-aur"
TARGET_REPO="${TARGET_REPO:-${DEFAULT_TARGET_REPO}}"
AUR_SSH_URL="${AUR_SSH_URL:-ssh://aur@aur.archlinux.org/${PACKAGE}.git}"

[[ -f "${PKGBUILD}" ]] || { echo "PKGBUILD tidak ditemukan: ${PKGBUILD}" >&2; exit 1; }
[[ -f "${ROOT_DIR}/VERSION" ]] || { echo "Berkas VERSION tidak ditemukan di ${ROOT_DIR}" >&2; exit 1; }
[[ -f "${EDITOR_DIR}/Cargo.toml" ]] || { echo "Cargo.toml tidak ditemukan di ${EDITOR_DIR}" >&2; exit 1; }
command -v python3 >/dev/null 2>&1 || { echo "python3 diperlukan untuk membaca Cargo.toml" >&2; exit 1; }

# ---------------------------------------------------------------- helper ----
# sed -i portabel (GNU vs BSD/macOS).
sed_inplace() {
  if sed --version >/dev/null 2>&1; then sed -i "$@"; else sed -i '' "$@"; fi
}

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

# Cetak satu array PKGBUILD, satu elemen per baris (dijalankan di subshell
# agar fungsi/variabel PKGBUILD tidak mencemari skrip ini).
pkgbuild_array() {
  ( cd "${PKGBUILD_DIR}" && bash -c 'source ./PKGBUILD; arr="$1[@]"; for v in "${!arr}"; do printf "%s\n" "$v"; done' _ "$1" )
}
pkgbuild_scalar() {
  ( cd "${PKGBUILD_DIR}" && bash -c 'source ./PKGBUILD; printf "%s" "${!1-}"' _ "$1" )
}

# Berkas sumber lokal = entri `source` tanpa skema URL/VCS (dan tanpa `::`).
is_local_source() {
  case "$1" in
    *://*|git+*|hg+*|svn+*|bzr+*) return 1 ;;
    *) return 0 ;;
  esac
}

# Emitter .SRCINFO cadangan — meniru urutan field `makepkg --printsrcinfo`
# untuk PKGBUILD paket tunggal (bukan split package).
emit_srcinfo_fallback() {
  local name key
  name="$(pkgbuild_scalar pkgname)"
  echo "pkgbase = ${name}"
  for key in pkgdesc pkgver pkgrel epoch url install changelog; do
    local val; val="$(pkgbuild_scalar "${key}")"
    [[ -n "${val}" ]] && printf '\t%s = %s\n' "${key}" "${val}"
  done
  for key in arch groups license checkdepends makedepends depends optdepends \
             provides conflicts replaces noextract options backup source validpgpkeys \
             md5sums sha1sums sha224sums sha256sums sha384sums sha512sums b2sums; do
    pkgbuild_array "${key}" 2>/dev/null | while IFS= read -r val; do
      [[ -n "${val}" ]] && printf '\t%s = %s\n' "${key}" "${val}"
    done
  done
  echo
  echo "pkgname = ${name}"
}

generate_srcinfo() {
  if command -v makepkg >/dev/null 2>&1; then
    echo "Membuat .SRCINFO lewat makepkg"
    ( cd "${PKGBUILD_DIR}" && makepkg --printsrcinfo > .SRCINFO )
    return
  fi
  local runner=""
  if command -v docker >/dev/null 2>&1; then runner=docker
  elif command -v podman >/dev/null 2>&1; then runner=podman
  fi
  if [[ -n "${runner}" ]] && "${runner}" info >/dev/null 2>&1; then
    echo "Membuat .SRCINFO lewat container archlinux (${runner})"
    if "${runner}" run --rm -v "${PKGBUILD_DIR}:/pkg:ro" -w /pkg archlinux:base-devel \
         bash -c 'useradd -m b >/dev/null 2>&1; su b -c "makepkg --printsrcinfo"' > "${PKGBUILD_DIR}/.SRCINFO"; then
      return
    fi
    echo "Container gagal; memakai emitter bawaan" >&2
  fi
  echo "makepkg tidak tersedia; membuat .SRCINFO dengan emitter bawaan"
  emit_srcinfo_fallback > "${PKGBUILD_DIR}/.SRCINFO"
}

# ------------------------------------------------------------- versi -----
VERSION="$(tr -d '[:space:]' < "${ROOT_DIR}/VERSION")"
CARGO_VERSION="$(cd "${EDITOR_DIR}" && python3 - <<'PY'
import pathlib, sys
try:
    import tomllib
except ModuleNotFoundError:
    try:
        import tomli as tomllib
    except ModuleNotFoundError:
        sys.exit("Modul Python tomllib/tomli diperlukan")
data = tomllib.loads(pathlib.Path("Cargo.toml").read_text())
print(data["workspace"]["package"]["version"])
PY
)"

if [[ -z "${VERSION}" || -z "${CARGO_VERSION}" ]]; then
  echo "Gagal menentukan versi (VERSION='${VERSION}', Cargo='${CARGO_VERSION}')" >&2
  exit 1
fi
if [[ "${VERSION}" != "${CARGO_VERSION}" ]]; then
  echo "VERSION (${VERSION}) tidak sama dengan ducad-editor/Cargo.toml (${CARGO_VERSION})." >&2
  echo "Samakan dulu (bump versi), lalu jalankan ulang." >&2
  exit 1
fi
if ! [[ "${VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Format versi tidak valid untuk pkgver: '${VERSION}' (harus X.Y.Z, pkgver tidak boleh berisi '-')" >&2
  exit 1
fi

if [[ "${TAG_CHECK}" == 1 ]]; then
  echo "Memeriksa tag v${VERSION} di origin"
  if ! git -C "${ROOT_DIR}" ls-remote --exit-code --tags origin "refs/tags/v${VERSION}" >/dev/null 2>&1; then
    echo "Tag v${VERSION} belum ada di remote origin; PKGBUILD membangun dari tag itu." >&2
    echo "Buat dan push tag dulu (git tag v${VERSION} && git push origin v${VERSION}), atau pakai --skip-tag-check." >&2
    exit 1
  fi
fi

# ---------------------------------------------------------- PKGBUILD -----
echo "Memperbarui PKGBUILD (${PACKAGE}) ke versi ${VERSION}"
sed_inplace -e "s/^pkgver=.*/pkgver=${VERSION}/" "${PKGBUILD}"
sed_inplace -e "s/^pkgrel=.*/pkgrel=1/" "${PKGBUILD}"

# Hitung ulang sha256sums: 'SKIP' untuk sumber VCS/URL, hash nyata untuk
# berkas lokal di samping PKGBUILD. Blok sha256sums ditulis ulang dalam
# format multi-baris standar.
LOCAL_FILES=()
SUMS=""
while IFS= read -r src; do
  [[ -z "${src}" ]] && continue
  if is_local_source "${src}"; then
    [[ -f "${PKGBUILD_DIR}/${src}" ]] || { echo "Berkas sumber lokal hilang: ${PKGBUILD_DIR}/${src}" >&2; exit 1; }
    LOCAL_FILES+=("${src}")
    SUMS="${SUMS}$(sha256_file "${PKGBUILD_DIR}/${src}")"$'\n'
  else
    SUMS="${SUMS}SKIP"$'\n'
  fi
done < <(pkgbuild_array source)

python3 - "${PKGBUILD}" "${SUMS}" <<'PY'
import re, sys, pathlib
path = pathlib.Path(sys.argv[1])
sums = [s for s in sys.argv[2].splitlines() if s]
indent = " " * len("sha256sums=(")
body = ("\n" + indent).join(f"'{s}'" for s in sums)
text = path.read_text()
new = re.sub(r"^sha256sums=\((?:.|\n)*?\)\n", f"sha256sums=({body})\n", text, count=1, flags=re.M)
if new == text and "sha256sums=(" not in text:
    sys.exit("Blok sha256sums=( ... ) tidak ditemukan di PKGBUILD")
path.write_text(new)
PY

generate_srcinfo
grep -q "pkgver = ${VERSION}" "${PKGBUILD_DIR}/.SRCINFO" || { echo ".SRCINFO tidak memuat pkgver ${VERSION}" >&2; exit 1; }

# ------------------------------------------------------- repo AUR --------
if [[ ! -d "${TARGET_REPO}/.git" ]]; then
  if [[ -e "${TARGET_REPO}" ]]; then
    echo "Peringatan: ${TARGET_REPO} ada tapi bukan repo git; hanya menyalin berkas." >&2
    mkdir -p "${TARGET_REPO}"
  else
    echo "Meng-clone repo AUR ${AUR_SSH_URL} ke ${TARGET_REPO}"
    if ! git clone "${AUR_SSH_URL}" "${TARGET_REPO}"; then
      echo "Clone gagal. Pastikan kunci SSH sudah didaftarkan di akun AUR (https://aur.archlinux.org/account)." >&2
      exit 1
    fi
  fi
fi

echo "Menyalin PKGBUILD, .SRCINFO, dan berkas sumber lokal ke ${TARGET_REPO}"
cp "${PKGBUILD}" "${TARGET_REPO}/PKGBUILD"
cp "${PKGBUILD_DIR}/.SRCINFO" "${TARGET_REPO}/.SRCINFO"
COPIED=(PKGBUILD .SRCINFO)
for f in ${LOCAL_FILES[@]+"${LOCAL_FILES[@]}"}; do
  cp "${PKGBUILD_DIR}/${f}" "${TARGET_REPO}/${f}"
  COPIED+=("${f}")
done
for f in "${PKGBUILD_DIR}"/*.install; do
  [[ -f "${f}" ]] || continue
  cp "${f}" "${TARGET_REPO}/$(basename "${f}")"
  COPIED+=("$(basename "${f}")")
done

if [[ -d "${TARGET_REPO}/.git" ]]; then
  (
    cd "${TARGET_REPO}"
    git add -- "${COPIED[@]}"
    if git diff --cached --quiet; then
      echo "Tidak ada perubahan untuk di-commit."
    else
      git commit -m "${PACKAGE} ${VERSION}"
      if [[ "${PUSH}" == 1 ]]; then
        echo "Push ke AUR (master)"
        git push origin HEAD:master
      else
        echo "--no-push: commit dibuat, push dilewati."
      fi
    fi
  )
fi

echo "Pembaruan paket AUR selesai (${PACKAGE} ${VERSION})."

#!/usr/bin/env bash
set -euo pipefail

# Rilis versi baru DUCAD dari berkas VERSION.
#
# Pemakaian: scripts/release-tag.sh [--no-push] [--no-bump]
#
# Langkah:
#   1. Baca versi dari VERSION (root repo), validasi format semver.
#   2. Bila tag `v<versi>` sudah ada (lokal atau di `origin`), artinya VERSION
#      belum dinaikkan: patch dinaikkan +1 otomatis (0.7.9 -> 0.7.10) dan
#      ditulis ke VERSION. Dengan --no-bump, script berhenti dengan error.
#   3. Set `[workspace.package].version` di ducad-editor/Cargo.toml,
#      MARKETING_VERSION/CURRENT_PROJECT_VERSION di DUCAD.xcodeproj, dan
#      segarkan Cargo.lock untuk crate workspace.
#   4. Commit VERSION, Cargo.toml, Cargo.lock, dan project.pbxproj saja (perubahan lain di
#      working tree tidak ikut).
#   5. Buat tag annotated `v<versi>`, lalu push cabang saat ini dan tag ke
#      `origin` — tag `v*` memicu workflow release.yml.

PUSH=1
BUMP=1
for arg in "$@"; do
  case "$arg" in
    --no-push) PUSH=0 ;;
    --no-bump) BUMP=0 ;;
    -h|--help) sed -n '4,20p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "argumen tidak dikenal: $arg" >&2; exit 2 ;;
  esac
done

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

CARGO_TOML="ducad-editor/Cargo.toml"
CARGO_LOCK="ducad-editor/Cargo.lock"
PBXPROJ="ducad-editor/DUCAD.xcodeproj/project.pbxproj"

VERSION="$(tr -d '[:space:]' < VERSION)"
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "VERSION tidak valid: '$VERSION'" >&2
  exit 1
fi
BRANCH="$(git rev-parse --abbrev-ref HEAD)"

# Tag yang sudah ada di remote diambil sekali saja (hindari ls-remote berulang).
REMOTE_TAGS="$(git ls-remote --tags origin 'refs/tags/v*' 2>/dev/null | awk '{print $2}' || true)"
tag_exists() {
  git rev-parse -q --verify "refs/tags/$1" >/dev/null ||
    grep -qx "refs/tags/$1" <<<"$REMOTE_TAGS"
}

# Bila tag untuk VERSION sudah ada (VERSION belum dinaikkan sejak rilis
# terakhir), naikkan patch +1 otomatis sampai menemukan tag yang belum dipakai.
# Matikan dengan --no-bump.
if tag_exists "v$VERSION"; then
  if [[ "$BUMP" == 0 ]]; then
    echo "tag v$VERSION sudah ada — naikkan VERSION dulu (atau jalankan tanpa --no-bump)" >&2
    exit 1
  fi
  if [[ "$VERSION" == *-* ]]; then
    echo "tag v$VERSION sudah ada dan versi pre-release tidak bisa dinaikkan otomatis — edit VERSION manual" >&2
    exit 1
  fi
  OLD_VERSION="$VERSION"
  IFS=. read -r MAJ MIN PAT <<<"$VERSION"
  while tag_exists "v$MAJ.$MIN.$PAT"; do
    PAT=$((PAT + 1))
  done
  VERSION="$MAJ.$MIN.$PAT"
  echo "$VERSION" > VERSION
  echo "==> tag v$OLD_VERSION sudah ada, VERSION dinaikkan otomatis: $OLD_VERSION -> $VERSION"
fi
TAG="v$VERSION"

# Ganti baris `version = ...` pertama di dalam seksi [workspace.package].
awk -v ver="$VERSION" '
  /^\[/ { in_pkg = ($0 == "[workspace.package]") }
  in_pkg && !done && /^version[[:space:]]*=/ { print "version = \"" ver "\""; done = 1; next }
  { print }
  END { if (!done) exit 3 }
' "$CARGO_TOML" > "$CARGO_TOML.tmp" || {
  rm -f "$CARGO_TOML.tmp"
  echo "gagal menemukan version di [workspace.package] $CARGO_TOML" >&2
  exit 1
}
mv "$CARGO_TOML.tmp" "$CARGO_TOML"
echo "==> $CARGO_TOML: version = \"$VERSION\""

# Xcode: Info.plist macOS & iOS membaca $(MARKETING_VERSION) dan
# $(CURRENT_PROJECT_VERSION), jadi cukup set keduanya di semua konfigurasi.
# Build number default = angka versi digabung (0.5.2 -> 52), bisa ditimpa
# lewat env BUILD; harus lebih besar dari build number tertinggi saat ini
# karena App Store menolak build number yang tidak naik.
CUR_BUILD="$(sed -n 's/.*CURRENT_PROJECT_VERSION = \([0-9]*\);.*/\1/p' "$PBXPROJ" | sort -n | tail -1)"
BUILD="${BUILD:-$(echo "${VERSION%%-*}" | tr -d . | sed 's/^0*//')}"
if ! [[ "$BUILD" =~ ^[0-9]+$ ]] || (( BUILD <= ${CUR_BUILD:-0} )); then
  echo "build number $BUILD tidak lebih besar dari $CUR_BUILD — set BUILD=<n> manual" >&2
  exit 1
fi
sed -i '' \
  -e "s/MARKETING_VERSION = [^;]*;/MARKETING_VERSION = $VERSION;/" \
  -e "s/CURRENT_PROJECT_VERSION = [^;]*;/CURRENT_PROJECT_VERSION = $BUILD;/" \
  "$PBXPROJ"
echo "==> $PBXPROJ: MARKETING_VERSION = $VERSION, CURRENT_PROJECT_VERSION = $BUILD"

if command -v cargo >/dev/null 2>&1; then
  (cd ducad-editor && cargo update --workspace --offline --quiet)
  echo "==> $CARGO_LOCK diperbarui"
else
  echo "peringatan: cargo tidak ditemukan, $CARGO_LOCK tidak diperbarui" >&2
fi

git add VERSION "$CARGO_TOML" "$CARGO_LOCK" "$PBXPROJ"
if git diff --cached --quiet -- VERSION "$CARGO_TOML" "$CARGO_LOCK" "$PBXPROJ"; then
  echo "==> tidak ada perubahan versi untuk di-commit"
else
  git commit -m "chore: release $TAG" -- VERSION "$CARGO_TOML" "$CARGO_LOCK" "$PBXPROJ"
fi

git tag -a "$TAG" -m "DUCAD $TAG"
echo "==> tag $TAG dibuat di $(git rev-parse --short HEAD)"

if [[ "$PUSH" == 1 ]]; then
  git push origin "$BRANCH"
  git push origin "$TAG"
  echo "==> $BRANCH dan $TAG di-push ke origin"
else
  echo "==> --no-push: jalankan 'git push origin $BRANCH && git push origin $TAG' bila siap"
fi


cd ducad-editor && ./build_macos.sh 
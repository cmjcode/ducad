#!/usr/bin/env bash
set -euo pipefail

# Rilis versi baru DUCAD dari berkas VERSION.
#
# Pemakaian: scripts/release-tag.sh [--no-push]
#
# Langkah:
#   1. Baca versi dari VERSION (root repo), validasi format semver.
#   2. Pastikan tag `v<versi>` belum ada (lokal maupun di remote `origin`).
#   3. Set `[workspace.package].version` di ducad-editor/Cargo.toml dan
#      segarkan Cargo.lock untuk crate workspace.
#   4. Commit VERSION, Cargo.toml, dan Cargo.lock saja (perubahan lain di
#      working tree tidak ikut).
#   5. Buat tag annotated `v<versi>`, lalu push cabang saat ini dan tag ke
#      `origin` — tag `v*` memicu workflow release.yml.

PUSH=1
for arg in "$@"; do
  case "$arg" in
    --no-push) PUSH=0 ;;
    -h|--help) sed -n '4,16p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "argumen tidak dikenal: $arg" >&2; exit 2 ;;
  esac
done

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

CARGO_TOML="ducad-editor/Cargo.toml"
CARGO_LOCK="ducad-editor/Cargo.lock"

VERSION="$(tr -d '[:space:]' < VERSION)"
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "VERSION tidak valid: '$VERSION'" >&2
  exit 1
fi
TAG="v$VERSION"
BRANCH="$(git rev-parse --abbrev-ref HEAD)"

if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
  echo "tag $TAG sudah ada secara lokal — naikkan VERSION dulu" >&2
  exit 1
fi
if git ls-remote --exit-code --tags origin "refs/tags/$TAG" >/dev/null 2>&1; then
  echo "tag $TAG sudah ada di origin — naikkan VERSION dulu" >&2
  exit 1
fi

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

if command -v cargo >/dev/null 2>&1; then
  (cd ducad-editor && cargo update --workspace --offline --quiet)
  echo "==> $CARGO_LOCK diperbarui"
else
  echo "peringatan: cargo tidak ditemukan, $CARGO_LOCK tidak diperbarui" >&2
fi

git add VERSION "$CARGO_TOML" "$CARGO_LOCK"
if git diff --cached --quiet -- VERSION "$CARGO_TOML" "$CARGO_LOCK"; then
  echo "==> tidak ada perubahan versi untuk di-commit"
else
  git commit -m "chore: release $TAG" -- VERSION "$CARGO_TOML" "$CARGO_LOCK"
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

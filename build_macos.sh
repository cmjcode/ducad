#!/usr/bin/env bash
set -euo pipefail

# Root launcher untuk DUCAD build_macos.sh.
# Kredensial Apple dibaca dari berkas `.env` lokal (di-ignore git);
# lihat `.env.example` untuk daftar variabel yang didukung.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [ -f "$SCRIPT_DIR/.env" ]; then
    set -a
    # shellcheck disable=SC1091
    source "$SCRIPT_DIR/.env"
    set +a
fi

if [ -f "$SCRIPT_DIR/VERSION" ]; then
    export VERSION
    VERSION="$(tr -d ' \r\n' < "$SCRIPT_DIR/VERSION")"
fi

chmod +x "$SCRIPT_DIR/ducad-editor/build_macos.sh" 2>/dev/null || true
exec "$SCRIPT_DIR/ducad-editor/build_macos.sh" "$@"

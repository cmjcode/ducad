#!/bin/bash
# Scripts launcher for DUCAD clean_no_occt.sh
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
chmod +x "$ROOT_DIR/ducad-editor/clean_no_occt.sh" 2>/dev/null || true
exec "$ROOT_DIR/ducad-editor/clean_no_occt.sh" "$@"

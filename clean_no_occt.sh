#!/bin/bash
# Root launcher for DUCAD clean_no_occt.sh
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
chmod +x "$SCRIPT_DIR/ducad-editor/clean_no_occt.sh" 2>/dev/null || true
exec "$SCRIPT_DIR/ducad-editor/clean_no_occt.sh" "$@"

#!/bin/bash
# Root launcher for DUCAD build_macos.sh
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
chmod +x "$SCRIPT_DIR/ducad-editor/build_macos.sh" 2>/dev/null || true
exec "$SCRIPT_DIR/ducad-editor/build_macos.sh" "$@"

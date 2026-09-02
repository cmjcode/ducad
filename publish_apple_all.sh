#!/bin/bash
# Root launcher for DUCAD publish_apple_all.sh
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
chmod +x "$SCRIPT_DIR/ducad-editor/publish_apple_all.sh" 2>/dev/null || true
exec "$SCRIPT_DIR/ducad-editor/publish_apple_all.sh" "$@"

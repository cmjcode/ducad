#!/bin/bash

export APPLE_ID='nunung.pamungkas@vneu.co.id'
export APPLE_TEAM_ID='YD4J5Z6A4G'
export APPLE_BUNDLE_ID='id.ducad.studio'
export PASSWORD='Simbok21AMIDAMA'
export APPLE_IDENTITY='Developer ID Application: PT. VNEU TEKNOLOGI INDONESIA (YD4J5Z6A4G)'
export APPLE_IDENTITY_INS='Developer ID Installer: PT. VNEU TEKNOLOGI INDONESIA (YD4J5Z6A4G)'

# Root launcher for DUCAD build_macos.sh
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [ -f "$SCRIPT_DIR/VERSION" ]; then
    export VERSION="$(tr -d ' \r\n' < "$SCRIPT_DIR/VERSION")"
fi

chmod +x "$SCRIPT_DIR/ducad-editor/build_macos.sh" 2>/dev/null || true
exec "$SCRIPT_DIR/ducad-editor/build_macos.sh" "$@"

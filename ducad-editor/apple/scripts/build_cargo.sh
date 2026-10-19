#!/bin/bash
# ==============================================================================
# DUCAD Xcode Cargo Build Bridge
# ==============================================================================
# This script is called by Xcode's Run Script Phase to build the Rust binary
# for the selected SDK, architecture, and configuration, then copies the binary
# and dSYM into the Xcode build products folder for seamless archiving and
# App Store Connect upload.
# ==============================================================================

set -e

# Prepend standard cargo & toolchain paths
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APPLE_DIR="$(dirname "$SCRIPT_DIR")"
EDITOR_DIR="$(dirname "$APPLE_DIR")"
cd "$EDITOR_DIR"

echo "=================================================="
echo "🚀 [DUCAD Xcode Bridge] Starting Cargo Build Phase"
echo "  Configuration : ${CONFIGURATION:-Release}"
echo "  Platform      : ${PLATFORM_NAME:-macosx}"
echo "  Architectures : ${ARCHS:-arm64}"
echo "  Action        : ${ACTION:-build}"
echo "  Built Products: ${BUILT_PRODUCTS_DIR:-dist}"
echo "=================================================="

# 1. Verify Rust & Cargo
if ! command -v cargo &>/dev/null; then
    echo "error: Cargo not found in PATH ($PATH). Please ensure Rust is installed." >&2
    exit 1
fi

# 2. Determine Rust target
RUST_TARGET=""
case "${PLATFORM_NAME:-macosx}" in
    iphoneos)
        RUST_TARGET="aarch64-apple-ios"
        ;;
    iphonesimulator)
        if [[ "${ARCHS:-arm64}" == *"arm64"* ]]; then
            RUST_TARGET="aarch64-apple-ios-sim"
        else
            RUST_TARGET="x86_64-apple-ios"
        fi
        ;;
    macosx)
        if [[ "${ARCHS:-arm64}" == *"arm64"* ]]; then
            RUST_TARGET="aarch64-apple-darwin"
        else
            RUST_TARGET="x86_64-apple-darwin"
        fi
        ;;
    *)
        echo "warning: Unknown platform ${PLATFORM_NAME}, defaulting to host arch" >&2
        RUST_TARGET="aarch64-apple-darwin"
        ;;
esac

echo "🎯 Selected Rust Target: $RUST_TARGET"


# 3. iOS SDK (kernel truck: Rust murni, tanpa pre-build C++/cmake)
# Detect & export iOS SDK paths for toolchain files before unsetting Xcode variables
export SDK_IPHONEOS="$(xcrun --sdk iphoneos --show-sdk-path 2>/dev/null || echo "${SDKROOT:-/Applications/Xcode.app/Contents/Developer/Platforms/iPhoneOS.platform/Developer/SDKs/iPhoneOS.sdk}")"
export SDK_SIMULATOR="$(xcrun --sdk iphonesimulator --show-sdk-path 2>/dev/null || echo "/Applications/Xcode.app/Contents/Developer/Platforms/iPhoneSimulator.platform/Developer/SDKs/iPhoneSimulator.sdk")"

# CRITICAL FIX for Xcode + Rust cross-compilation:
# When Xcode builds an iOS scheme, it injects SDKROOT pointing to the iPhoneOS SDK
# and sets target-specific CFLAGS/LDFLAGS. Procedural macro crates (such as bytemuck_derive,
# serde_derive, thiserror-impl) and build scripts must compile for the macOS HOST architecture.
# If SDKROOT remains set to an iOS SDK, clang tries to link host proc-macros against the iOS SDK,
# leading to linker failures or dyld rejection (error[E0463]: can't find crate for `bytemuck_derive`).
# Unsetting SDKROOT, CFLAGS, CXXFLAGS, LDFLAGS, CC, CXX allows cargo/rustc to compile host
# tools against the macOS host SDK, while rustc's `--target` flag automatically supplies
# the iOS SDK for the target binaries.
unset SDKROOT
unset CFLAGS
unset CXXFLAGS
unset LDFLAGS
unset CC
unset CXX

# 4. Run Cargo Build
CARGO_FLAGS="--target $RUST_TARGET -p ducad-app"
BUILD_SUBDIR="release"

if [ "${CONFIGURATION}" = "Debug" ] && [ "${ACTION}" != "install" ]; then
    BUILD_SUBDIR="debug"
else
    CARGO_FLAGS="$CARGO_FLAGS --release"
fi

echo "📦 Running: cargo build $CARGO_FLAGS"
cargo build $CARGO_FLAGS

SOURCE_BIN="$EDITOR_DIR/target/$RUST_TARGET/$BUILD_SUBDIR/ducad"
if [ ! -f "$SOURCE_BIN" ]; then
    echo "error: Compiled binary not found at $SOURCE_BIN" >&2
    exit 1
fi

# 5. Copy Binary to Xcode's Built Products Destination
if [ -n "$BUILT_PRODUCTS_DIR" ] && [ -n "$EXECUTABLE_PATH" ]; then
    DEST_BIN="$BUILT_PRODUCTS_DIR/$EXECUTABLE_PATH"
    echo "📋 Copying binary to $DEST_BIN"
    mkdir -p "$(dirname "$DEST_BIN")"
    cp "$SOURCE_BIN" "$DEST_BIN"
    chmod +x "$DEST_BIN"
    
    # 6. Generate dSYM for App Store Crash Symbolication
    if [ -n "$DWARF_DSYM_FOLDER_PATH" ] && [ -n "$DWARF_DSYM_FILE_NAME" ] && command -v dsymutil &>/dev/null; then
        echo "🔍 Generating dSYM at $DWARF_DSYM_FOLDER_PATH/$DWARF_DSYM_FILE_NAME"
        mkdir -p "$DWARF_DSYM_FOLDER_PATH"
        dsymutil "$SOURCE_BIN" -o "$DWARF_DSYM_FOLDER_PATH/$DWARF_DSYM_FILE_NAME" 2>/dev/null || true
    fi

    # 7. App bundle sanitization & asset hygiene
    APP_BUNDLE_PATH="$(dirname "$DEST_BIN")"
    if [[ "$PLATFORM_NAME" == "macosx" ]]; then
        APP_BUNDLE_PATH="$(dirname "$APP_BUNDLE_PATH")" # Contents -> DUCAD.app
    fi

    # Clean quarantine / extended attributes from source assets if present
    if [ -d "$EDITOR_DIR/assets" ]; then
        xattr -cr "$EDITOR_DIR/assets" 2>/dev/null || true
        find "$EDITOR_DIR/assets" -name ".DS_Store" -delete 2>/dev/null || true
    fi

    # Binary baru saja diganti: buang tanda tangan lama agar Xcode wajib menandatangani
    # ulang (CodeSign). Tanpa ini bundle bisa terpasang dengan _CodeSignature basi
    # dan iPad menolak: "sign the executable with a valid certificate".
    if [ -d "$APP_BUNDLE_PATH" ]; then
        rm -rf "$APP_BUNDLE_PATH/_CodeSignature" "$APP_BUNDLE_PATH/Contents/_CodeSignature" 2>/dev/null || true
    fi

    # Ensure app bundle is clean of com.apple.quarantine, provenance, and .DS_Store
    if [ -d "$APP_BUNDLE_PATH" ]; then
        echo "🧹 Sanitizing app bundle at $APP_BUNDLE_PATH (removing quarantine, extended attributes & .DS_Store)..."
        rm -rf "$APP_BUNDLE_PATH/Contents/Resources/assets" 2>/dev/null || true
        rm -rf "$APP_BUNDLE_PATH/assets" 2>/dev/null || true
        xattr -cr "$APP_BUNDLE_PATH" 2>/dev/null || true
        find "$APP_BUNDLE_PATH" -name ".DS_Store" -delete 2>/dev/null || true
    fi
fi

echo "✅ [DUCAD Xcode Bridge] Cargo build phase finished successfully!"

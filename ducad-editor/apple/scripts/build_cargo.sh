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

# 3. Configure iOS toolchain & OCCT if compiling for iOS/iPadOS
TOOLCHAIN_FILE="$EDITOR_DIR/crates/ducad-kernel/ios/ios-toolchain.cmake"
if [[ "$RUST_TARGET" == *"apple-ios"* ]]; then
    export CMAKE_POLICY_VERSION_MINIMUM="3.5"
    if [ -f "$TOOLCHAIN_FILE" ]; then
        export CMAKE_TOOLCHAIN_FILE_aarch64_apple_ios="$TOOLCHAIN_FILE"
        export CMAKE_TOOLCHAIN_FILE_aarch64_apple_ios_sim="$TOOLCHAIN_FILE"
        export CMAKE_TOOLCHAIN_FILE_x86_64_apple_ios="$TOOLCHAIN_FILE"
    fi

    # Pre-build OCCT if needed (same serial install safety as build_ipad.sh)
    OCCT_TARGET_DIR="$EDITOR_DIR/target/$RUST_TARGET/OCCT"
    OCCT_LIB_STEP="$OCCT_TARGET_DIR/lib/libTKDESTEP.a"
    OCCT_SRC_DIR="$HOME/.cargo/registry/src"
    OCCT_SYS_DIR=$(find "$OCCT_SRC_DIR" -name "OCCT" -type d 2>/dev/null | grep "occt-sys" | head -n1 || true)
    
    if [ ! -f "$OCCT_LIB_STEP" ] && [ -n "$OCCT_SYS_DIR" ] && [ -d "$OCCT_SYS_DIR" ]; then
        echo "⚙️ Pre-building OCCT for target $RUST_TARGET..."
        BUILD_DIR="$OCCT_TARGET_DIR/build"
        mkdir -p "$BUILD_DIR"
        TOOLCHAIN_FLAG=""
        if [ -f "$TOOLCHAIN_FILE" ]; then
            TOOLCHAIN_FLAG="-DCMAKE_TOOLCHAIN_FILE=$TOOLCHAIN_FILE"
        fi
        PATCH_DIR="$(dirname "$OCCT_SYS_DIR")/patch"
        cmake "$OCCT_SYS_DIR" -B "$BUILD_DIR" \
            -DBUILD_PATCH="$PATCH_DIR" \
            -DBUILD_LIBRARY_TYPE=Static \
            -DBUILD_MODULE_ApplicationFramework=FALSE \
            -DBUILD_MODULE_Draw=FALSE \
            -DUSE_D3D=FALSE -DUSE_DRACO=FALSE -DUSE_EIGEN=FALSE -DUSE_FFMPEG=FALSE \
            -DUSE_FREEIMAGE=FALSE -DUSE_FREETYPE=FALSE -DUSE_GLES2=FALSE -DUSE_OPENGL=FALSE \
            -DUSE_OPENVR=FALSE -DUSE_RAPIDJSON=FALSE -DUSE_TBB=FALSE -DUSE_TCL=FALSE \
            -DUSE_TK=FALSE -DUSE_VTK=FALSE -DUSE_XLIB=FALSE \
            -DINSTALL_DIR_LIB=lib -DINSTALL_DIR_INCLUDE=include \
            $TOOLCHAIN_FLAG \
            -DCMAKE_INSTALL_PREFIX="$OCCT_TARGET_DIR" \
            -DCMAKE_BUILD_TYPE=Release
        
        NCPUS=$(sysctl -n hw.ncpu 2>/dev/null || echo 4)
        cmake --build "$BUILD_DIR" --config Release --parallel "$NCPUS"
        cmake --install "$BUILD_DIR"
        echo "✅ OCCT pre-build complete for $RUST_TARGET"
    fi
fi

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

    # 7. Copy application assets if present
    APP_DIR="$(dirname "$DEST_BIN")"
    if [[ "$PLATFORM_NAME" == "macosx" ]]; then
        APP_DIR="$(dirname "$APP_DIR")/Resources"
    fi
    mkdir -p "$APP_DIR"
    if [ -d "$EDITOR_DIR/assets" ]; then
        cp -r "$EDITOR_DIR/assets" "$APP_DIR/assets" 2>/dev/null || true
    fi
fi

echo "✅ [DUCAD Xcode Bridge] Cargo build phase finished successfully!"

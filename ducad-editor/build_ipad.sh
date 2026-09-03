#!/bin/bash
# ==============================================================================
# DUCAD iPadOS / iOS Build & Packaging Script
# ==============================================================================
# Usage: ./build_ipad.sh [ACTION] [OPTIONS]
#
# Actions:
#   device (default)  - Build binary & create .app bundle for physical iPad (aarch64-apple-ios)
#   sim / simulator   - Build binary & create .app bundle for iPad Simulator
#   run-sim / launch  - Build simulator app, boot iPad Simulator & launch DUCAD
#   archive           - Build and create .xcarchive (ready for Xcode Organizer)
#   ipa / package     - Build and create signed/ad-hoc .ipa for TestFlight / Distribution
#   publish           - Build .ipa and upload to App Store Connect / TestFlight
#   check             - Quick cargo check for iOS target
#   clean             - Clean iOS build artifacts
#   install-deps      - Install required Rust iOS targets (aarch64-apple-ios, aarch64-apple-ios-sim)
#
# Options:
#   --debug           - Build debug configuration (default: release)
#   --bundle-id <ID>  - Custom bundle ID (default: $APPLE_BUNDLE_ID or id.jayuda.ducad.ios)
#   --sim-name <NAME> - Target simulator name (default: auto-detect booted or iPad Pro / Air)
#   --identity <ID>   - Code signing identity
#   --profile <PATH>  - Provisioning profile (.mobileprovision)
#   --help, -h        - Show this help message
# ==============================================================================

set -e

# --- 1. Determine Directories ---
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -f "$SCRIPT_DIR/Cargo.toml" ]; then
    EDITOR_DIR="$SCRIPT_DIR"
    ROOT_DIR="$(dirname "$SCRIPT_DIR")"
elif [ -d "$SCRIPT_DIR/ducad-editor" ]; then
    EDITOR_DIR="$SCRIPT_DIR/ducad-editor"
    ROOT_DIR="$SCRIPT_DIR"
else
    EDITOR_DIR="$PWD"
    ROOT_DIR="$PWD"
fi

# Load .env if present
if [ -f "$ROOT_DIR/.env" ]; then
    set -a
    source "$ROOT_DIR/.env"
    set +a
elif [ -f "$EDITOR_DIR/.env" ]; then
    set -a
    source "$EDITOR_DIR/.env"
    set +a
fi

cd "$EDITOR_DIR"

# --- 2. Color Output Helpers ---
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
BOLD='\033[1m'
NC='\033[0m' # No Color

print_status() {
    echo -e "${BLUE}[INFO]${NC} $1" >&2
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1" >&2
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1" >&2
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

# --- 3. Configuration & Metadata ---
APP_NAME="DUCAD"
BIN_NAME="ducad"
PKG_NAME="ducad-app"
VERSION=$(grep '^version' Cargo.toml 2>/dev/null | head -n1 | cut -d '"' -f2 || echo "0.1.0")
BUILD_NUMBER="${BUILD_NUMBER:-1}"
if command -v git &>/dev/null && git rev-parse --is-inside-work-tree &>/dev/null; then
    GIT_COMMITS=$(git rev-list --count HEAD 2>/dev/null || echo "1")
    BUILD_NUMBER="$GIT_COMMITS"
fi

APPLE_BUNDLE_ID="${APPLE_BUNDLE_ID}"
APPLE_TEAM_ID="${APPLE_TEAM_ID}"
APPLE_ID="${APPLE_ID}"
APPLE_PASSWORD="${APPLE_PASSWORD}"

IOS_ARM_TARGET="aarch64-apple-ios"
HOST_ARCH=$(uname -m)
if [ "$HOST_ARCH" = "arm64" ]; then
    IOS_SIM_TARGET="aarch64-apple-ios-sim"
else
    IOS_SIM_TARGET="x86_64-apple-ios"
fi

BUILD_MODE="release"
CARGO_BUILD_FLAG="--release"
BUILD_SUBDIR="release"

DIST_IOS_DIR="$EDITOR_DIR/dist/ios"
TOOLCHAIN_FILE="$EDITOR_DIR/crates/ducad-kernel/ios/ios-toolchain.cmake"
ENTITLEMENTS_FILE="$EDITOR_DIR/crates/ducad-app/ios/DUCAD-iOS.entitlements"

ACTION="device"
CUSTOM_SIM_NAME=""
CUSTOM_IDENTITY="${APPLE_IDENTITY:-}"
PROVISIONING_PROFILE="${PROVISIONING_PROFILE:-}"

# --- 4. Parse Arguments ---
show_help() {
    echo -e "${CYAN}${BOLD}📱 DUCAD iPadOS / iOS Build & Packaging Tool${NC}"
    echo "=================================================="
    echo "Version: $VERSION (Build $BUILD_NUMBER)"
    echo "Bundle ID: $APPLE_BUNDLE_ID"
    echo ""
    echo "Usage: $0 [ACTION] [OPTIONS]"
    echo ""
    echo -e "${BOLD}Actions:${NC}"
    echo "  device           - Build binary & create .app bundle for physical iPad (default)"
    echo "  sim, simulator   - Build binary & create .app bundle for iPad Simulator"
    echo "  run-sim, launch  - Build & launch on iPad Simulator automatically"
    echo "  archive          - Create .xcarchive bundle (openable in Xcode Organizer)"
    echo "  ipa, package     - Create signed or ad-hoc .ipa for iPad distribution"
    echo "  publish          - Upload .ipa directly to App Store Connect / TestFlight"
    echo "  check            - Run fast cargo check for aarch64-apple-ios target"
    echo "  clean            - Clean dist/ios and iOS build caches"
    echo "  install-deps     - Add required rustup iOS targets"
    echo ""
    echo -e "${BOLD}Options:${NC}"
    echo "  --debug          - Compile in debug mode instead of release"
    echo "  --bundle-id <ID> - Override Apple Bundle Identifier"
    echo "  --sim-name <NAME>- Specify Simulator device name (e.g., 'iPad Pro 11-inch (M4)')"
    echo "  --identity <ID>  - Signing identity (e.g., 'Apple Development: ...')"
    echo "  --profile <PATH> - Path to .mobileprovision file"
    echo "  --help, -h       - Show this help message"
    echo ""
    echo -e "${BOLD}Examples:${NC}"
    echo "  $0 run-sim                    # Build & run directly in iPad Simulator"
    echo "  $0 ipa                        # Build .app and package into dist/ios/DUCAD-$VERSION.ipa"
    echo "  $0 archive                    # Generate DUCAD.xcarchive for Xcode Organizer"
    echo "  $0 publish                    # Build and upload to TestFlight automatically"
    echo ""
}

while [[ $# -gt 0 ]]; do
    case $1 in
        device|build)
            ACTION="device"
            shift
            ;;
        sim|simulator)
            ACTION="sim"
            shift
            ;;
        run-sim|launch)
            ACTION="run-sim"
            shift
            ;;
        archive|xcarchive)
            ACTION="archive"
            shift
            ;;
        ipa|package)
            ACTION="ipa"
            shift
            ;;
        publish|testflight)
            ACTION="publish"
            shift
            ;;
        check)
            ACTION="check"
            shift
            ;;
        clean)
            ACTION="clean"
            shift
            ;;
        install-deps|deps)
            ACTION="install-deps"
            shift
            ;;
        --debug)
            BUILD_MODE="debug"
            CARGO_BUILD_FLAG=""
            BUILD_SUBDIR="debug"
            shift
            ;;
        --bundle-id)
            APPLE_BUNDLE_ID="$2"
            shift 2
            ;;
        --sim-name)
            CUSTOM_SIM_NAME="$2"
            shift 2
            ;;
        --identity)
            CUSTOM_IDENTITY="$2"
            shift 2
            ;;
        --profile)
            PROVISIONING_PROFILE="$2"
            shift 2
            ;;
        --help|-h)
            show_help
            exit 0
            ;;
        *)
            print_error "Unknown argument: $1"
            show_help
            exit 1
            ;;
    esac
done

# --- 5. Environment & Toolchain Setup ---
ensure_occt_built() {
    local target="${1:-$IOS_ARM_TARGET}"
    local occt_target_dir="$EDITOR_DIR/target/$target/OCCT"
    local occt_lib_step="$occt_target_dir/lib/libTKDESTEP.a"
    local occt_src_dir="$HOME/.cargo/registry/src"
    local occt_sys_dir
    occt_sys_dir=$(find "$occt_src_dir" -name "OCCT" -type d 2>/dev/null | grep "occt-sys" | head -n1 || true)
    
    if [ ! -f "$occt_lib_step" ] && [ -n "$occt_sys_dir" ] && [ -d "$occt_sys_dir" ]; then
        print_status "Pre-building OCCT for target $target (serial install to avoid CMake APFS race condition)..."
        local build_dir="$occt_target_dir/build"
        mkdir -p "$build_dir"
        local toolchain_flag=""
        if [ -f "$TOOLCHAIN_FILE" ]; then
            toolchain_flag="-DCMAKE_TOOLCHAIN_FILE=$TOOLCHAIN_FILE"
        fi
        local patch_dir
        patch_dir="$(dirname "$occt_sys_dir")/patch"
        cmake "$occt_sys_dir" -B "$build_dir" \
            -DBUILD_PATCH="$patch_dir" \
            -DBUILD_LIBRARY_TYPE=Static \
            -DBUILD_MODULE_ApplicationFramework=FALSE \
            -DBUILD_MODULE_Draw=FALSE \
            -DUSE_D3D=FALSE -DUSE_DRACO=FALSE -DUSE_EIGEN=FALSE -DUSE_FFMPEG=FALSE \
            -DUSE_FREEIMAGE=FALSE -DUSE_FREETYPE=FALSE -DUSE_GLES2=FALSE -DUSE_OPENGL=FALSE \
            -DUSE_OPENVR=FALSE -DUSE_RAPIDJSON=FALSE -DUSE_TBB=FALSE -DUSE_TCL=FALSE \
            -DUSE_TK=FALSE -DUSE_VTK=FALSE -DUSE_XLIB=FALSE \
            -DINSTALL_DIR_LIB=lib -DINSTALL_DIR_INCLUDE=include \
            $toolchain_flag \
            -DCMAKE_INSTALL_PREFIX="$occt_target_dir" \
            -DCMAKE_BUILD_TYPE=Release
        
        local ncpus
        ncpus=$(sysctl -n hw.ncpu 2>/dev/null || echo 4)
        cmake --build "$build_dir" --config Release --parallel "$ncpus"
        cmake --install "$build_dir"
        print_success "OCCT pre-build complete for $target!"
    fi
}

setup_toolchain() {
    local target="${1:-$IOS_ARM_TARGET}"
    print_status "Configuring iOS CMake & Cargo toolchain..."
    
    export CMAKE_POLICY_VERSION_MINIMUM="3.5"
    if [ -f "$TOOLCHAIN_FILE" ]; then
        export CMAKE_TOOLCHAIN_FILE_aarch64_apple_ios="$TOOLCHAIN_FILE"
        export CMAKE_TOOLCHAIN_FILE_aarch64_apple_ios_sim="$TOOLCHAIN_FILE"
        export CMAKE_TOOLCHAIN_FILE_x86_64_apple_ios="$TOOLCHAIN_FILE"
    fi
    
    # Check developer SDK
    if command -v xcrun &>/dev/null; then
        export SDK_IPHONEOS=$(xcrun --sdk iphoneos --show-sdk-path 2>/dev/null || true)
        export SDK_SIMULATOR=$(xcrun --sdk iphonesimulator --show-sdk-path 2>/dev/null || true)
    fi

    ensure_occt_built "$target"
}

install_dependencies() {
    print_status "Installing Rust iOS targets and dependencies..."
    rustup target add "$IOS_ARM_TARGET" "$IOS_SIM_TARGET" || true
    print_success "Rust iOS targets installed successfully!"
}

# --- 6. Helper: Generate Info.plist ---
generate_info_plist() {
    local target_plist="$1"
    local template_file="$EDITOR_DIR/crates/ducad-app/ios/Info.plist.template"
    
    print_status "Generating Info.plist for bundle $APPLE_BUNDLE_ID..."
    
    if [ -f "$template_file" ]; then
        sed -e "s/com.ducad.app/$APPLE_BUNDLE_ID/g" \
            -e "s/<string>1<\/string>/<string>$BUILD_NUMBER<\/string>/g" \
            -e "s/<string>0.1.0<\/string>/<string>$VERSION<\/string>/g" \
            "$template_file" > "$target_plist"
    else
        cat <<EOF > "$target_plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundleDisplayName</key>
    <string>$APP_NAME</string>
    <key>CFBundleIdentifier</key>
    <string>$APPLE_BUNDLE_ID</string>
    <key>CFBundleExecutable</key>
    <string>$BIN_NAME</string>
    <key>CFBundleVersion</key>
    <string>$BUILD_NUMBER</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>MinimumOSVersion</key>
    <string>15.0</string>
    <key>CFBundleSupportedPlatforms</key>
    <array>
        <string>iPhoneOS</string>
    </array>
    <key>UIDeviceFamily</key>
    <array>
        <integer>2</integer>
    </array>
    <key>DTPlatformName</key>
    <string>iphoneos</string>
    <key>DTPlatformVersion</key>
    <string>15.0</string>
    <key>DTSDKName</key>
    <string>iphoneos</string>
    <key>DTCompiler</key>
    <string>com.apple.compilers.llvm.clang.1_0</string>
    <key>LSRequiresIPhoneOS</key>
    <true/>
    <key>UIRequiredDeviceCapabilities</key>
    <array>
        <string>arm64</string>
    </array>
    <key>UISupportedInterfaceOrientations</key>
    <array>
        <string>UIInterfaceOrientationLandscapeLeft</string>
        <string>UIInterfaceOrientationLandscapeRight</string>
    </array>
    <key>UISupportedInterfaceOrientations~ipad</key>
    <array>
        <string>UIInterfaceOrientationLandscapeLeft</string>
        <string>UIInterfaceOrientationLandscapeRight</string>
        <string>UIInterfaceOrientationPortrait</string>
    </array>
    <key>UIFileSharingEnabled</key>
    <true/>
    <key>LSSupportsOpeningDocumentsInPlace</key>
    <true/>
</dict>
</plist>
EOF
    fi
    print_success "Info.plist created at $target_plist"
}

# --- 7. Helper: Generate App Icons ---
generate_app_icons() {
    local app_bundle_dir="$1"
    local icon_src="$EDITOR_DIR/assets/icon.png"
    
    if [ ! -f "$icon_src" ]; then
        icon_src="$EDITOR_DIR/crates/ducad-app/assets/icon.png"
    fi
    
    if [ -f "$icon_src" ] && command -v sips &>/dev/null; then
        print_status "Generating iPad icon assets from $icon_src..."
        sips -z 1024 1024 "$icon_src" --out "$app_bundle_dir/AppIcon1024x1024.png" &>/dev/null || true
        sips -z 167 167   "$icon_src" --out "$app_bundle_dir/AppIcon83.5x83.5@2x.png" &>/dev/null || true
        sips -z 152 152   "$icon_src" --out "$app_bundle_dir/AppIcon76x76@2x.png" &>/dev/null || true
        sips -z 76 76     "$icon_src" --out "$app_bundle_dir/AppIcon76x76@1x.png" &>/dev/null || true
        sips -z 40 40     "$icon_src" --out "$app_bundle_dir/AppIcon20x20@2x.png" &>/dev/null || true
        sips -z 58 58     "$icon_src" --out "$app_bundle_dir/AppIcon29x29@2x.png" &>/dev/null || true
        sips -z 80 80     "$icon_src" --out "$app_bundle_dir/AppIcon40x40@2x.png" &>/dev/null || true
        cp "$icon_src" "$app_bundle_dir/AppIcon.png" 2>/dev/null || true
    fi
}

# --- 8. Helper: Code Signing ---
codesign_bundle() {
    local bundle_path="$1"
    local is_sim="$2"
    
    if ! command -v codesign &>/dev/null; then
        print_warning "codesign command not found. Skipping code signing."
        return 0
    fi
    
    print_status "Code signing $bundle_path..."
    
    local sign_id="$CUSTOM_IDENTITY"
    if [ -z "$sign_id" ] || [[ "$sign_id" == *"Developer ID"* ]]; then
        # Cari sertifikat Apple Distribution terlebih dahulu, lalu Apple Development
        sign_id=$(security find-identity -v -p codesigning 2>/dev/null | grep "Apple Distribution" | head -n1 | sed -E 's/.*"([^"]+)".*/\1/' || true)
        if [ -z "$sign_id" ]; then
            sign_id=$(security find-identity -v -p codesigning 2>/dev/null | grep "Apple Development" | head -n1 | sed -E 's/.*"([^"]+)".*/\1/' || echo "-")
        fi
    fi
    
    local entitlements_flag=""
    if [ -f "$ENTITLEMENTS_FILE" ] && [ "$is_sim" != "true" ]; then
        entitlements_flag="--entitlements $ENTITLEMENTS_FILE --generate-entitlement-der"
    fi
    
    # Tandatangani binary utama di dalam bundle terlebih dahulu
    if [ -f "$bundle_path/$BIN_NAME" ]; then
        codesign --force --sign "${sign_id:--}" --timestamp=none "$bundle_path/$BIN_NAME" 2>/dev/null || true
    fi
    
    if [ -n "$sign_id" ] && [ "$sign_id" != "-" ]; then
        print_status "Signing with identity: $sign_id"
        codesign --force --sign "$sign_id" --timestamp=none $entitlements_flag "$bundle_path" 2>/dev/null || {
            print_warning "Signing with $sign_id failed. Falling back to Ad-Hoc (-)..."
            codesign --force --sign "-" --timestamp=none "$bundle_path/$BIN_NAME" 2>/dev/null || true
            codesign --force --sign "-" --timestamp=none "$bundle_path"
        }
    else
        print_status "Signing with Ad-Hoc signature (-)..."
        codesign --force --sign "-" --timestamp=none "$bundle_path/$BIN_NAME" 2>/dev/null || true
        codesign --force --sign "-" --timestamp=none "$bundle_path"
    fi
    
    print_success "Code signing complete!"
}

# --- 9. Action Implementations ---

do_check() {
    print_status "Running cargo check for iOS target ($IOS_ARM_TARGET)..."
    setup_toolchain
    cargo check --target "$IOS_ARM_TARGET" -p "$PKG_NAME"
    print_success "cargo check passed for iOS target!"
}

do_clean() {
    print_status "Cleaning iOS build artifacts..."
    rm -rf "$DIST_IOS_DIR"
    rm -rf "$EDITOR_DIR/target/$IOS_ARM_TARGET"
    rm -rf "$EDITOR_DIR/target/$IOS_SIM_TARGET"
    print_success "iOS build artifacts cleaned!"
}

build_app_bundle() {
    local target="$1"
    local is_sim="$2"
    local app_dir_name="$APP_NAME.app"
    if [ "$is_sim" = "true" ]; then
        app_dir_name="$APP_NAME-Simulator.app"
    fi
    
    local app_bundle="$DIST_IOS_DIR/$app_dir_name"
    
    print_status "Building $PKG_NAME for target $target ($BUILD_MODE)..."
    setup_toolchain "$target"
    
    cargo build $CARGO_BUILD_FLAG -p "$PKG_NAME" --target "$target"
    
    local bin_source="$EDITOR_DIR/target/$target/$BUILD_SUBDIR/$BIN_NAME"
    if [ ! -f "$bin_source" ]; then
        print_error "Binary not found at $bin_source"
        exit 1
    fi
    
    mkdir -p "$DIST_IOS_DIR"
    rm -rf "$app_bundle"
    mkdir -p "$app_bundle"
    
    print_status "Creating .app bundle structure at $app_bundle..."
    cp "$bin_source" "$app_bundle/$BIN_NAME"
    chmod +x "$app_bundle/$BIN_NAME"
    
    generate_info_plist "$app_bundle/Info.plist"
    generate_app_icons "$app_bundle"
    
    if [ -d "$EDITOR_DIR/assets" ]; then
        cp -r "$EDITOR_DIR/assets" "$app_bundle/assets" 2>/dev/null || true
    fi
    
    if [ -n "$PROVISIONING_PROFILE" ] && [ -f "$PROVISIONING_PROFILE" ]; then
        print_status "Embedding provisioning profile..."
        cp "$PROVISIONING_PROFILE" "$app_bundle/embedded.mobileprovision"
    fi
    
    # Strip com.apple.quarantine and extended attributes from bundle
    xattr -cr "$app_bundle" 2>/dev/null || true
    find "$app_bundle" -name ".DS_Store" -delete 2>/dev/null || true
    
    codesign_bundle "$app_bundle" "$is_sim"
    
    print_success "App bundle created successfully at: $app_bundle"
}

do_run_sim() {
    print_status "Preparing iPad Simulator execution..."
    
    if ! command -v xcrun &>/dev/null; then
        print_error "Xcode command line tools (xcrun) not found."
        exit 1
    fi
    
    build_app_bundle "$IOS_SIM_TARGET" "true"
    local app_bundle="$DIST_IOS_DIR/$APP_NAME-Simulator.app"
    
    # Find iPad simulator
    local sim_id=""
    if [ -n "$CUSTOM_SIM_NAME" ]; then
        sim_id=$(xcrun simctl list devices available | grep "$CUSTOM_SIM_NAME" | head -n1 | grep -oE '\([A-F0-9-]{36}\)' | tr -d '()')
    fi
    
    if [ -z "$sim_id" ]; then
        # Check if any iPad is already Booted
        sim_id=$(xcrun simctl list devices | grep -i "iPad" | grep "Booted" | head -n1 | grep -oE '\([A-F0-9-]{36}\)' | tr -d '()')
    fi
    
    if [ -z "$sim_id" ]; then
        # Pick first available iPad simulator
        sim_id=$(xcrun simctl list devices available | grep -i "iPad" | head -n1 | grep -oE '\([A-F0-9-]{36}\)' | tr -d '()')
    fi
    
    if [ -z "$sim_id" ]; then
        print_error "No available iPad simulator found. Please install an iPad simulator via Xcode."
        exit 1
    fi
    
    print_status "Targeting Simulator ID: $sim_id"
    
    # Boot simulator if not booted
    local sim_state
    sim_state=$(xcrun simctl list devices | grep "$sim_id" | grep -o "Booted" || true)
    if [ "$sim_state" != "Booted" ]; then
        print_status "Booting iPad Simulator..."
        xcrun simctl boot "$sim_id" 2>/dev/null || true
    fi
    
    # Open Simulator GUI
    open -a Simulator 2>/dev/null || true
    
    print_status "Installing $APP_NAME on Simulator..."
    xcrun simctl install "$sim_id" "$app_bundle"
    
    print_status "Launching $APPLE_BUNDLE_ID on Simulator..."
    xcrun simctl launch "$sim_id" "$APPLE_BUNDLE_ID"
    
    print_success "🎉 DUCAD is now running on iPad Simulator!"
}

do_archive() {
    build_app_bundle "$IOS_ARM_TARGET" "false"
    local app_bundle="$DIST_IOS_DIR/$APP_NAME.app"
    
    local archive_dir="$DIST_IOS_DIR/$APP_NAME.xcarchive"
    print_status "Creating .xcarchive bundle at $archive_dir..."
    rm -rf "$archive_dir"
    mkdir -p "$archive_dir/Products/Applications"
    
    cp -R "$app_bundle" "$archive_dir/Products/Applications/$APP_NAME.app"
    
    local date_str
    date_str=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    
    cat <<EOF > "$archive_dir/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>ApplicationProperties</key>
    <dict>
        <key>ApplicationPath</key>
        <string>Applications/$APP_NAME.app</string>
        <key>CFBundleIdentifier</key>
        <string>$APPLE_BUNDLE_ID</string>
        <key>CFBundleShortVersionString</key>
        <string>$VERSION</string>
        <key>CFBundleVersion</key>
        <string>$BUILD_NUMBER</string>
        <key>SigningIdentity</key>
        <string>${CUSTOM_IDENTITY:-Apple Development}</string>
        <key>Team</key>
        <string>$APPLE_TEAM_ID</string>
    </dict>
    <key>ArchiveVersion</key>
    <integer>2</integer>
    <key>CreationDate</key>
    <date>$date_str</date>
    <key>Name</key>
    <string>$APP_NAME</string>
    <key>SchemeName</key>
    <string>$APP_NAME</string>
</dict>
</plist>
EOF
    
    print_success "🎉 .xcarchive created successfully at: $archive_dir"
    echo "ℹ️  You can double-click this archive to open Xcode Organizer (Window > Organizer) and distribute to App Store Connect."
}

do_ipa() {
    build_app_bundle "$IOS_ARM_TARGET" "false"
    local app_bundle="$DIST_IOS_DIR/$APP_NAME.app"
    
    local payload_dir="$DIST_IOS_DIR/Payload"
    local ipa_output="$DIST_IOS_DIR/$APP_NAME-$VERSION.ipa"
    
    print_status "Packaging .ipa payload..."
    rm -rf "$payload_dir" "$ipa_output"
    mkdir -p "$payload_dir"
    
    cp -R "$app_bundle" "$payload_dir/$APP_NAME.app"
    
    print_status "Compressing into $ipa_output..."
    (cd "$DIST_IOS_DIR" && zip -qry "$APP_NAME-$VERSION.ipa" Payload)
    rm -rf "$payload_dir"
    
    print_success "🎉 .ipa generated successfully at: $ipa_output"
    
    # Also create .xcarchive for convenience
    do_archive >/dev/null 2>&1 || true
}

do_publish() {
    print_status "Preparing IPA for App Store Connect / TestFlight publication..."
    do_ipa
    
    local ipa_file="$DIST_IOS_DIR/$APP_NAME-$VERSION.ipa"
    if [ ! -f "$ipa_file" ]; then
        print_error "IPA file not found at $ipa_file"
        exit 1
    fi
    
    if [ -z "$APPLE_ID" ] || [ -z "$APPLE_PASSWORD" ]; then
        print_error "Missing APPLE_ID or APPLE_PASSWORD credentials in .env file."
        exit 1
    fi
    
    local team_arg=()
    if [ -n "$APPLE_TEAM_ID" ]; then
        team_arg=(--team-id "$APPLE_TEAM_ID")
    fi
    
    print_status "Validating IPA with App Store Connect..."
    xcrun altool --validate-app \
        -f "$ipa_file" \
        -t ios \
        -u "$APPLE_ID" \
        -p "$APPLE_PASSWORD" \
        "${team_arg[@]}" || {
            print_warning "Validation returned warnings/errors. Proceeding with upload if possible..."
        }
        
    print_status "Uploading IPA to TestFlight / App Store Connect..."
    xcrun altool --upload-app \
        -f "$ipa_file" \
        -t ios \
        -u "$APPLE_ID" \
        -p "$APPLE_PASSWORD" \
        "${team_arg[@]}"
        
    print_success "🚀 IPA uploaded successfully to TestFlight / App Store Connect!"
}

# --- 10. Main Dispatcher ---
main() {
    echo -e "${CYAN}${BOLD}🚀 DUCAD iPad Build Pipeline (${ACTION})${NC}"
    echo "=============================================="
    
    case $ACTION in
        install-deps)
            install_dependencies
            ;;
        check)
            do_check
            ;;
        clean)
            do_clean
            ;;
        sim)
            build_app_bundle "$IOS_SIM_TARGET" "true"
            ;;
        run-sim)
            do_run_sim
            ;;
        device)
            build_app_bundle "$IOS_ARM_TARGET" "false"
            ;;
        archive)
            do_archive
            ;;
        ipa)
            do_ipa
            ;;
        publish)
            do_publish
            ;;
        *)
            print_error "Unknown action: $ACTION"
            show_help
            exit 1
            ;;
    esac
    
    echo ""
    print_status "📦 Output directory ($DIST_IOS_DIR):"
    if [ -d "$DIST_IOS_DIR" ]; then
        find "$DIST_IOS_DIR" -maxdepth 2 \( -name "*.ipa" -o -name "*.app" -o -name "*.xcarchive" \) 2>/dev/null | while read -r item; do
            size=$(du -sh "$item" 2>/dev/null | cut -f1)
            echo "  📁 $item ($size)"
        done
    fi
    echo ""
    print_success "✨ Process completed!"
}

main "$@"

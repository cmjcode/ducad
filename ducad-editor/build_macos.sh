#!/bin/bash
# Quick build script for DUCAD Editor (macOS & multiplatform)
# Usage: ./build_macos.sh [platform] [options]
# Platforms: macos, macos-pkg, ipad, ipad-publish, publish-all, linux, windows, all


set -e

# Determine project directory (root or ducad-editor)
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

APP_NAME="DUCAD"
# Ambil versi dari Cargo.toml supaya sinkron
VERSION=$(grep '^version' Cargo.toml 2>/dev/null | head -n1 | cut -d '"' -f2 || echo "0.1.0")

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if required tools are installed
check_dependencies() {
    print_status "Checking dependencies..."
    
    if ! command -v cargo &> /dev/null; then
        print_error "Cargo is not installed. Please install Rust first."
        exit 1
    fi
    
    if ! command -v make &> /dev/null; then
        print_error "Make is not installed. Please install make."
        exit 1
    fi
    
    print_success "All dependencies are available!"
}

# Show help
show_help() {
    echo -e "${CYAN}🛠️  DUCAD Editor Build Script${NC}"
    echo "=============================="
    echo "Version: $VERSION"
    echo ""
    echo "Usage: $0 [PLATFORM] [OPTIONS]"
    echo ""
    echo "Platforms:"
    echo "  macos        - Build macOS binary + .app + .dmg (Developer ID / Local)"
    echo "  macos-pkg    - Build macOS .app lalu signed .pkg (Mac App Store / Distribusi)"
    echo "  ipad         - Build iPadOS target & binary"
    echo "  ipad-publish - Build + Publish iPad app to App Store Connect / TestFlight"
    echo "  publish-all  - Build & Publish iPadOS (.ipa) + macOS (.pkg) ke App Store Connect"
    echo "  linux        - Build + package Linux (x86_64)"
    echo "  windows      - Build + package Windows (x86_64)"
    echo "  all          - Release build semua platform"
    echo ""
    echo "Options:"
    echo "  --deps       - Install build dependencies and targets first"
    echo "  --clean      - Clean previous builds before building"
    echo "  --help, -h   - Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 macos          # Build macOS .app & .dmg"
    echo "  $0 macos-pkg      # Build macOS signed .pkg for App Store"
    echo "  $0 publish-all    # Publish iPadOS + macOS to App Store Connect"
    echo "  $0 linux --clean  # Clean and build Linux"
    echo "  $0 all --deps     # Install deps and build all"
    echo ""
}

# Parse command line arguments
PLATFORM="macos"
INSTALL_DEPS=false
CLEAN_FIRST=false

while [[ $# -gt 0 ]]; do
    case $1 in
        macos|macos-pkg|ipad|ipad-publish|publish-all|apple-publish|linux|windows|all)
            PLATFORM="$1"
            shift
            ;;
        --deps)
            INSTALL_DEPS=true
            shift
            ;;
        --clean)
            CLEAN_FIRST=true
            shift
            ;;
        --help|-h)
            show_help
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
done

# Main build function
main() {
    print_status "Starting build for platform: $PLATFORM (Working Directory: $EDITOR_DIR)"
    
    check_dependencies
    
    # Install dependencies if requested
    if [ "$INSTALL_DEPS" = true ]; then
        print_status "Installing build dependencies..."
        make install-deps
    fi
    
    # Clean if requested
    if [ "$CLEAN_FIRST" = true ]; then
        print_status "Cleaning previous builds..."
        make clean
    fi
    
    # Build based on platform
    case $PLATFORM in
        macos)
            print_status "Building macOS binary and App Store ready bundle for DUCAD..."
            print_status "Setting up environment variables for code signing..."
            
            # NOTE: DMG TANPA SANDBOX -> JANGAN set APPLE_APP_IDENTITY di sini.
            # Jika ingin membuat versi App Store (sandbox), jalankan:
            #   APPLE_APP_IDENTITY='3rd Party Mac Developer Application: PT. VNEU TEKNOLOGI INDONESIA (YD4J5Z6A4G)' ./build_apple.sh macos-pkg
            unset APPLE_APP_IDENTITY
            
            echo "✅ Environment variables set for code signing"
            echo "📝 Note: For notarization, manually set NOTARIZE=1 or run ./notarize.sh"
            echo "ℹ️  DMG akan dibangun dengan entitlements non-sandbox (Developer ID)."
            
            make clean
            make bundle-macos
            if [ "${NOTARIZE:-0}" = "1" ] && [ -f "./notarize.sh" ]; then
                sh notarize.sh
            fi
            print_success "macOS build completed!"
            ;;
        macos-pkg)
            # Export environment variables for code signing
            export APPLE_ID="${APPLE_ID}"
            export APPLE_TEAM_ID="${APPLE_TEAM_ID}"
            export APPLE_BUNDLE_ID="${APPLE_BUNDLE_ID}"
            export PASSWORD="${PASSWORD}"
            export APPLE_PASSWORD="${APPLE_PASSWORD}"
            export APPLE_IDENTITY="${APPLE_IDENTITY}"
            export APPLE_IDENTITY_INS="${APPLE_IDENTITY_INS}"
            export APPLE_APP_IDENTITY="${APPLE_APP_IDENTITY}"
            
            print_status "Building macOS .app + signed .pkg for DUCAD"
            if [ -z "$APPLE_IDENTITY_INS" ] && [ -z "$APPLE_IDENTITY" ]; then
                print_warning "APPLE_IDENTITY belum diset."
            fi
            if [ -z "$APPLE_BUNDLE_ID" ]; then
                print_warning "APPLE_BUNDLE_ID belum diset (contoh: id.jayuda.ducad)"
            fi
            make pkg-macos-store || {
                print_error "Gagal membuat pkg. Pastikan env & provisioning profile benar."
                exit 1
            }
            print_success "macOS pkg build completed!"
            ;;
        ipad)
            print_status "Building DUCAD iPadOS package (.app / .ipa / .xcarchive)..."
            ./build_ipad.sh ipa
            print_success "iPad build completed!"
            ;;
        ipad-publish)
            print_status "Building and publishing DUCAD iPadOS to App Store Connect / TestFlight..."
            ./build_ipad.sh publish
            print_success "iPad publishing completed!"
            ;;
        publish-all|apple-publish)
            print_status "Building & Publishing Universal Purchase (iPadOS + macOS)..."
            ./publish_apple_all.sh
            print_success "Universal publish completed!"
            ;;
        linux)
            print_status "Building Linux binaries..."
            make bundle-linux
            print_success "Linux build completed!"
            ;;
        windows)
            print_status "Building Windows binaries..."
            make bundle-windows
            print_success "Windows build completed!"
            ;;
        all)
            print_status "Building for all platforms..."
            make release
            print_success "All platform builds completed!"
            ;;
        *)
            print_error "Unknown platform: $PLATFORM"
            exit 1
            ;;
    esac
    
    # Show build results
    echo ""
    print_success "🎉 Build completed successfully!"
    echo ""
    print_status "📦 Generated files in $EDITOR_DIR/dist:"
    
    if [ -d "dist" ]; then
        find dist -type f \( -name "*.dmg" -o -name "*.pkg" -o -name "*.app" -o -name "*.tar.gz" -o -name "*.zip" -o -name "ducad*" \) 2>/dev/null | while read -r file; do
            size=$(ls -lh "$file" 2>/dev/null | awk '{print $5}')
            echo "  📁 $file ($size)"
        done
    else
        print_warning "No distribution files found. Build may have failed."
    fi
    
    echo ""
    print_status "✨ Ready for distribution!"
}

# Run main function
main "$@"

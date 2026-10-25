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

# Ambil versi dari VERSION file sebagai single source of truth
if [ -f "$ROOT_DIR/VERSION" ]; then
    VERSION=$(tr -d ' \r\n' < "$ROOT_DIR/VERSION")
elif [ -f "$EDITOR_DIR/VERSION" ]; then
    VERSION=$(tr -d ' \r\n' < "$EDITOR_DIR/VERSION")
elif [ -f "VERSION" ]; then
    VERSION=$(tr -d ' \r\n' < "VERSION")
elif [ -n "$VERSION" ]; then
    VERSION="$VERSION"
else
    VERSION=$(grep '^version' Cargo.toml 2>/dev/null | head -n1 | cut -d '"' -f2 || echo "0.1.0")
fi
export VERSION

# Build number diambil dari VERSION dengan menghilangkan titik dan leading zero
if [ -z "$BUILD_NUMBER" ]; then
    BUILD_NUMBER=$(echo "$VERSION" | tr -d '.' | sed 's/^0*//')
    [ -z "$BUILD_NUMBER" ] && BUILD_NUMBER="1"
fi
export BUILD_NUMBER

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

# Synchronize all related project files with VERSION and BUILD_NUMBER
sync_project_versions() {
    print_status "Menyelaraskan seluruh versi terkait dengan VERSION ($VERSION) & BUILD ($BUILD_NUMBER)..."

    # Pastikan file VERSION konsisten di root dan ducad-editor
    if [ -d "$ROOT_DIR" ]; then
        echo "$VERSION" > "$ROOT_DIR/VERSION"
    fi
    if [ -d "$EDITOR_DIR" ]; then
        echo "$VERSION" > "$EDITOR_DIR/VERSION"
    fi

    # 1. Update ducad-editor/Cargo.toml ([workspace.package] version)
    if [ -f "$EDITOR_DIR/Cargo.toml" ]; then
        python3 -c "
import re
path = '$EDITOR_DIR/Cargo.toml'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()
new_content = re.sub(
    r'(\[workspace\.package\][\s\S]*?version\s*=\s*\")[^\"]+(\")',
    r'\g<1>$VERSION\g<2>',
    content,
    count=1
)
if new_content != content:
    with open(path, 'w', encoding='utf-8') as f:
        f.write(new_content)
    print('  - Updated Cargo.toml [workspace.package] version to $VERSION')
"
    fi

    # 2. Update ducad-editor/crates/ducad-app/Cargo.toml ([package.metadata.bundle] version)
    local app_cargo="$EDITOR_DIR/crates/ducad-app/Cargo.toml"
    if [ -f "$app_cargo" ]; then
        python3 -c "
import re
path = '$app_cargo'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()
new_content = re.sub(
    r'(\[package\.metadata\.bundle\][\s\S]*?version\s*=\s*\")[^\"]+(\")',
    r'\g<1>$VERSION\g<2>',
    content,
    count=1
)
if new_content != content:
    with open(path, 'w', encoding='utf-8') as f:
        f.write(new_content)
    print('  - Updated crates/ducad-app/Cargo.toml [package.metadata.bundle] version to $VERSION')
"
    fi

    # 3. Update DUCAD.xcodeproj/project.pbxproj (MARKETING_VERSION & CURRENT_PROJECT_VERSION)
    local pbxproj="$EDITOR_DIR/DUCAD.xcodeproj/project.pbxproj"
    if [ -f "$pbxproj" ]; then
        python3 -c "
import re
path = '$pbxproj'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()
new_content = re.sub(
    r'MARKETING_VERSION\s*=\s*[^;]+;',
    'MARKETING_VERSION = $VERSION;',
    content
)
new_content = re.sub(
    r'CURRENT_PROJECT_VERSION\s*=\s*[^;]+;',
    'CURRENT_PROJECT_VERSION = $BUILD_NUMBER;',
    new_content
)
if new_content != content:
    with open(path, 'w', encoding='utf-8') as f:
        f.write(new_content)
    print('  - Updated DUCAD.xcodeproj MARKETING_VERSION to $VERSION and CURRENT_PROJECT_VERSION to $BUILD_NUMBER')
"
    fi

    # 4. Update apple/scripts/generate_project.py
    local gen_proj="$EDITOR_DIR/apple/scripts/generate_project.py"
    if [ -f "$gen_proj" ]; then
        python3 -c "
import re
path = '$gen_proj'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()
new_content = re.sub(
    r'MARKETING_VERSION\s*=\s*[^;]+;',
    'MARKETING_VERSION = {version};',
    content
)
new_content = re.sub(
    r'CURRENT_PROJECT_VERSION\s*=\s*[^;]+;',
    'CURRENT_PROJECT_VERSION = {build_number};',
    new_content
)
if new_content != content:
    with open(path, 'w', encoding='utf-8') as f:
        f.write(new_content)
    print('  - Updated apple/scripts/generate_project.py MARKETING_VERSION to {version} and CURRENT_PROJECT_VERSION to {build_number}')
"
    fi

    # 5. Update crates/ducad-app/ios/Info.plist.template
    local ios_tpl="$EDITOR_DIR/crates/ducad-app/ios/Info.plist.template"
    if [ -f "$ios_tpl" ]; then
        python3 -c "
import re
path = '$ios_tpl'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()
new_content = re.sub(
    r'(<key>CFBundleShortVersionString</key>\s*<string>)[^<]+(</string>)',
    r'\g<1>$VERSION\g<2>',
    content
)
new_content = re.sub(
    r'(<key>CFBundleVersion</key>\s*<string>)[^<]+(</string>)',
    r'\g<1>$BUILD_NUMBER\g<2>',
    new_content
)
if new_content != content:
    with open(path, 'w', encoding='utf-8') as f:
        f.write(new_content)
    print('  - Updated crates/ducad-app/ios/Info.plist.template CFBundleShortVersionString to $VERSION and CFBundleVersion to $BUILD_NUMBER')
"
    fi

    print_success "Sinkronisasi versi selesai (v$VERSION, build $BUILD_NUMBER)!"
}

# Upload built artifacts to GitHub release
upload_github_release() {
    if [ "$UPLOAD_RELEASE" != true ]; then
        print_status "Upload ke GitHub release dilewati (--no-upload atau SKIP_UPLOAD=1)."
        return 0
    fi

    echo ""
    print_status "🚀 Menyiapkan upload ke GitHub Release..."

    # Check dependencies: gh CLI
    if ! command -v gh &> /dev/null; then
        print_warning "GitHub CLI (gh) tidak terpasang. Lewati upload ke GitHub release."
        print_warning "Pasang gh via: brew install gh"
        return 0
    fi

    if ! gh auth status &> /dev/null; then
        print_warning "GitHub CLI (gh) belum terotentikasi. Lewati upload ke GitHub release."
        print_warning "Jalankan: gh auth login"
        return 0
    fi

    # Tentukan repository GitHub
    local github_repo="${GITHUB_REPO:-}"
    if [ -z "$github_repo" ]; then
        local remote_url
        remote_url=$(git config --get remote.origin.url 2>/dev/null || echo "")
        if [[ "$remote_url" =~ github\.com[:/]([^/]+/[^/.]+)(\.git)?$ ]]; then
            github_repo="${BASH_REMATCH[1]}"
        fi
    fi
    github_repo="${github_repo:-cmjcode/ducad}"

    # Tentukan tag release
    local release_tag="${RELEASE_TAG:-v$VERSION}"

    # Kumpulkan berkas artifact yang akan di-upload dari dist
    local candidate_files=()
    if [ -d "dist" ]; then
        while IFS= read -r -d '' file; do
            candidate_files+=("$file")
        done < <(find dist -type f \( -name "*.dmg" -o -name "*.pkg" -o -name "*.tar.gz" -o -name "*.zip" -o -name "*.msi" \) -print0 2>/dev/null)
    fi

    if [ ${#candidate_files[@]} -eq 0 ]; then
        print_warning "Tidak ada berkas rilis (.dmg, .pkg, .tar.gz, .zip, .msi) ditemukan di $EDITOR_DIR/dist. Lewati upload."
        return 0
    fi

    # Pastikan setiap artefak biner memiliki berkas .sha256
    local upload_files=()
    for file in "${candidate_files[@]}"; do
        upload_files+=("$file")
        local sha_file="${file}.sha256"
        if [ ! -f "$sha_file" ]; then
            print_status "Membuat checksum SHA-256 untuk $(basename "$file")..."
            (cd "$(dirname "$file")" && shasum -a 256 "$(basename "$file")" > "$(basename "$sha_file")")
        fi
        if [ -f "$sha_file" ]; then
            upload_files+=("$sha_file")
        fi
    done

    # Deduplikasi daftar upload
    local unique_files=()
    local seen=" "
    for f in "${upload_files[@]}"; do
        if [ -f "$f" ] && [[ ! "$seen" =~ " $f " ]]; then
            unique_files+=("$f")
            seen="$seen$f "
        fi
    done

    print_status "Target repo: $github_repo"
    print_status "Target release tag: $release_tag"
    print_status "Berkas yang akan diunggah (${#unique_files[@]} berkas):"
    for f in "${unique_files[@]}"; do
        local sz
        sz=$(ls -lh "$f" 2>/dev/null | awk '{print $5}')
        echo "  📦 $f ($sz)"
    done

    # Cek apakah release sudah ada di GitHub
    local is_draft=false
    if gh release view "$release_tag" --repo "$github_repo" >/dev/null 2>&1; then
        print_status "Release $release_tag ditemukan di GitHub."
        local draft_status
        draft_status=$(gh release view "$release_tag" --repo "$github_repo" --json isDraft -q '.isDraft' 2>/dev/null || echo "false")
        if [ "$draft_status" = "true" ]; then
            is_draft=true
        fi
    else
        print_status "Release $release_tag belum ada. Membuat release baru..."
        local target_args=()
        local head_sha
        head_sha=$(git rev-parse HEAD 2>/dev/null || echo "")
        if [ -n "$head_sha" ]; then
            target_args=(--target "$head_sha")
        fi

        if ! gh release create "$release_tag" \
            --repo "$github_repo" \
            --title "DUCAD $release_tag" \
            --generate-notes \
            "${target_args[@]}"; then
            if gh release view "$release_tag" --repo "$github_repo" >/dev/null 2>&1; then
                print_status "Release $release_tag sudah dibuat oleh proses lain, lanjut upload."
            else
                print_error "Gagal membuat release $release_tag di $github_repo."
                return 1
            fi
        else
            print_success "Release $release_tag berhasil dibuat."
        fi
    fi

    # Upload berkas dengan --clobber
    print_status "Mengunggah berkas ke release $release_tag..."
    if gh release upload "$release_tag" "${unique_files[@]}" --repo "$github_repo" --clobber; then
        print_success "🎉 Berhasil mengunggah semua berkas ke GitHub release $release_tag!"
    else
        print_error "Gagal mengunggah berkas ke release $release_tag."
        return 1
    fi

    # Publikasikan jika --publish diminta, atau beri petunjuk jika masih draft
    if [ "$PUBLISH_RELEASE" = true ]; then
        print_status "Mempublikasikan release $release_tag (--publish aktif)..."
        gh release edit "$release_tag" --repo "$github_repo" --draft=false --latest
        print_success "Release $release_tag telah dipublikasikan dan berstatus latest!"
    elif [ "$is_draft" = true ]; then
        echo ""
        print_status "ℹ️  Release $release_tag saat ini masih berstatus DRAFT."
        print_status "Untuk mempublikasikannya ke pengguna / updater, jalankan:"
        echo "   gh release edit $release_tag --draft=false --latest"
    fi
}

# Show help
show_help() {
    echo -e "${CYAN}🛠️  DUCAD Editor Build Script${NC}"
    echo "=============================="
    echo "Version: $VERSION (Build $BUILD_NUMBER)"
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
    echo "  sync         - Hanya sinkronisasi versi & build number seluruh proyek"
    echo ""
    echo "Options:"
    echo "  --deps       - Install build dependencies and targets first"
    echo "  --clean      - Clean previous builds before building"
    echo "  --upload     - Upload build artifacts to GitHub release (default: enabled)"
    echo "  --no-upload  - Skip uploading build artifacts to GitHub release"
    echo "  --publish    - Publish draft release to public/latest after upload"
    echo "  --tag <tag>  - Target GitHub release tag (default: v<VERSION>)"
    echo "  --help, -h   - Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 macos              # Build macOS .app & .dmg lalu upload ke GitHub release"
    echo "  $0 macos --no-upload  # Build tanpa upload ke GitHub release"
    echo "  $0 macos --publish    # Build, upload, dan publikasikan rilis draft"
    echo "  $0 macos-pkg          # Build macOS signed .pkg for App Store"
    echo "  $0 publish-all        # Publish iPadOS + macOS to App Store Connect"
    echo "  $0 sync               # Sinkronisasi VERSION & build number"
    echo "  $0 linux --clean      # Clean and build Linux"
    echo "  $0 all --deps         # Install deps and build all"
    echo ""
}

# Parse command line arguments
PLATFORM="macos"
INSTALL_DEPS=false
CLEAN_FIRST=false
UPLOAD_RELEASE=true
PUBLISH_RELEASE=false
CUSTOM_TAG=""

if [ "${SKIP_UPLOAD:-0}" = "1" ]; then
    UPLOAD_RELEASE=false
fi
if [ "${AUTO_PUBLISH:-0}" = "1" ] || [ "${PUBLISH:-0}" = "1" ]; then
    PUBLISH_RELEASE=true
fi

while [[ $# -gt 0 ]]; do
    case $1 in
        macos|macos-pkg|ipad|ipad-publish|publish-all|apple-publish|linux|windows|all|sync)
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
        --upload)
            UPLOAD_RELEASE=true
            shift
            ;;
        --no-upload)
            UPLOAD_RELEASE=false
            shift
            ;;
        --publish)
            PUBLISH_RELEASE=true
            shift
            ;;
        --tag)
            CUSTOM_TAG="$2"
            shift 2
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

if [ -n "$CUSTOM_TAG" ]; then
    RELEASE_TAG="$CUSTOM_TAG"
fi

# Main build function
main() {
    print_status "Starting build for platform: $PLATFORM (Version: $VERSION, Build: $BUILD_NUMBER, Working Directory: $EDITOR_DIR)"
    
    check_dependencies
    
    # Sync all version references across the project
    sync_project_versions
    
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
            export APPLE_PASSWORD="${APPLE_PASSWORD:-$PASSWORD}"
            
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
            export APPLE_PASSWORD="${APPLE_PASSWORD:-$PASSWORD}"
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
        sync)
            print_success "Sinkronisasi versi seluruh proyek selesai (v$VERSION, build $BUILD_NUMBER)!"
            exit 0
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

    # Upload to GitHub release
    upload_github_release
}

# Run main function
main "$@"

# Ganti background transparan menjadi solid (misal #1a1a1a atau #000000) dan matikan alpha channel
magick ducad-editor/assets/icon.png -background "#1a1a1a" -alpha remove -alpha off ducad-editor/assets/icon.png

# Sinkronkan juga ke ducad-app assets jika diperlukan
cp ducad-editor/assets/icon.png ducad-editor/crates/ducad-app/assets/icon.png

# Regenerate asset catalog
cd ducad-editor && make xcode-assets && cd ..

sips -g all ducad-editor/apple/Assets.xcassets/AppIcon.appiconset/icon-1024.png | grep -E "hasAlpha|samplesPerPixel"

# Hapus extended attributes (termasuk com.apple.quarantine) dan .DS_Store
xattr -cr ducad-editor/assets ducad-editor/crates/ducad-app/assets ducad-editor/apple/Assets.xcassets 2>/dev/null || true
find ducad-editor/apple/Assets.xcassets ducad-editor/assets -name ".DS_Store" -delete 2>/dev/null || true

############################################
# DUCAD Editor Makefile
# Cross-platform build (macOS / Linux / Windows / iOS)
# Includes macOS codesign, notarization, App Store pkg
############################################

.PHONY: all help install-deps clean create-dirs \
        build-macos build-linux build-windows \
        bundle-macos bundle-linux bundle-windows pkg-macos-store \
        build-ipad archive-ipad ipa-ipad publish-ipad \
        release build run dev test check fmt info notarize notarize-check

all: help

APP_NAME = DUCAD
BIN_NAME = ducad
PKG_NAME = ducad-app
VERSION  = $(shell grep '^version' Cargo.toml | head -n1 | cut -d'"' -f2)
RUST_VERSION = stable

# Targets
MACOS_X86_TARGET = x86_64-apple-darwin
MACOS_ARM_TARGET = aarch64-apple-darwin
MACOS_UNIVERSAL_TARGET = universal-apple-darwin
LINUX_X86_TARGET = x86_64-unknown-linux-gnu
LINUX_ARM_TARGET = aarch64-unknown-linux-gnu
WINDOWS_X86_TARGET = x86_64-pc-windows-gnu
WINDOWS_ARM_TARGET = aarch64-pc-windows-gnu
IOS_ARM_TARGET = aarch64-apple-ios
IOS_SIM_TARGET = aarch64-apple-ios-sim

# Dirs
BUILD_DIR = target
DIST_DIR  = dist
MACOS_DIR = $(DIST_DIR)/macos
LINUX_DIR = $(DIST_DIR)/linux
WINDOWS_DIR = $(DIST_DIR)/windows
IOS_DIR   = $(DIST_DIR)/ios

help:
	@echo "🛠️  DUCAD Build System"
	@echo "================================"
	@echo "Version: $(VERSION)"
	@echo ""
	@echo "Core Targets:"
	@echo "  install-deps       Install all Rust targets + tools"
	@echo "  build-macos        Build macOS binary"
	@echo "  bundle-macos       Create .app & .dmg (codesign/notarize optional)"
	@echo "  pkg-macos-store    Create signed .pkg (App Store / distribution)"
	@echo "  build-ipad         Build iPadOS target / XCFramework"
	@echo "  build-linux        Build Linux binaries"
	@echo "  bundle-linux       Create tarballs + AppDir structure"
	@echo "  build-windows      Build Windows binaries"
	@echo "  bundle-windows     Create zipped Windows binaries"
	@echo "  release            Clean + deps + all bundles"
	@echo "  notarize           Notarize existing .app and .dmg"
	@echo "  notarize-check     Check notarization status"
	@echo "Dev Helpers:"
	@echo "  run / dev / test / check / fmt / info"
	@echo "Environment (macOS signing/notarization):"
	@echo "  APPLE_IDENTITY='Developer ID Application: Name (TEAMID)'"
	@echo "  APPLE_IDENTITY_INS='Developer ID Installer: Name (TEAMID)'"
	@echo "  APPLE_BUNDLE_ID='id.jayuda.ducad'"
	@echo "  NOTARIZE=1 APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID"
	@echo "  PROVISIONING_PROFILE=path/to/AppStore.provisionprofile (for pkg)"
	@echo ""

install-deps:
	@echo "📦 Installing build dependencies..."
	rustup target add $(MACOS_ARM_TARGET) $(MACOS_X86_TARGET) \
		$(LINUX_X86_TARGET) $(LINUX_ARM_TARGET) \
		$(WINDOWS_X86_TARGET) $(WINDOWS_ARM_TARGET) \
		$(IOS_ARM_TARGET) $(IOS_SIM_TARGET) || true
	cargo install cargo-bundle || true
	cargo install cross || true
	@echo "✅ Dependencies installed."

clean:
	@echo "🧹 Cleaning..."
	rm -rf $(DIST_DIR)
	rm -rf $(BUILD_DIR)/release/bundle
	rm -rf $(BUILD_DIR)/$(MACOS_ARM_TARGET)/release/bundle
	rm -rf $(BUILD_DIR)/$(MACOS_X86_TARGET)/release/bundle
	@echo "✅ Clean done."

create-dirs:
	@mkdir -p $(MACOS_DIR) $(LINUX_DIR) $(WINDOWS_DIR) $(IOS_DIR)

build-macos: create-dirs
	@echo "🍎 Build macOS binary (-p $(PKG_NAME))"
	cargo build --release -p $(PKG_NAME)
	@mkdir -p $(MACOS_DIR)
	@if [ -f $(BUILD_DIR)/release/$(BIN_NAME) ]; then \
		cp $(BUILD_DIR)/release/$(BIN_NAME) $(MACOS_DIR)/$(BIN_NAME); \
	fi
	@echo "✅ macOS binary ready."

bundle-macos: create-dirs
	@echo "📱 Create macOS .app bundle"
	cargo bundle --release --bin $(BIN_NAME)
	@mkdir -p $(MACOS_DIR)
	@if [ -d "$(BUILD_DIR)/release/bundle/osx/$(APP_NAME).app" ]; then \
		rm -rf "$(MACOS_DIR)/$(APP_NAME).app"; \
		cp -R "$(BUILD_DIR)/release/bundle/osx/$(APP_NAME).app" "$(MACOS_DIR)/"; \
	elif [ -d "$(BUILD_DIR)/release/bundle/osx/ducad-app.app" ]; then \
		rm -rf "$(MACOS_DIR)/$(APP_NAME).app"; \
		cp -R "$(BUILD_DIR)/release/bundle/osx/ducad-app.app" "$(MACOS_DIR)/$(APP_NAME).app"; \
	elif [ -d "$(BUILD_DIR)/$(MACOS_ARM_TARGET)/release/bundle/osx/$(APP_NAME).app" ]; then \
		rm -rf "$(MACOS_DIR)/$(APP_NAME).app"; \
		cp -R "$(BUILD_DIR)/$(MACOS_ARM_TARGET)/release/bundle/osx/$(APP_NAME).app" "$(MACOS_DIR)/"; \
	elif [ -d "$(BUILD_DIR)/$(MACOS_ARM_TARGET)/release/bundle/osx/ducad-app.app" ]; then \
		rm -rf "$(MACOS_DIR)/$(APP_NAME).app"; \
		cp -R "$(BUILD_DIR)/$(MACOS_ARM_TARGET)/release/bundle/osx/ducad-app.app" "$(MACOS_DIR)/$(APP_NAME).app"; \
	fi
	@if [ -f "$(MACOS_DIR)/$(APP_NAME).app/Contents/Info.plist" ]; then \
		BUNDLE_ID=$${APPLE_BUNDLE_ID:-id.ducad.studio}; \
		plutil -replace CFBundleName -string "$(APP_NAME)" "$(MACOS_DIR)/$(APP_NAME).app/Contents/Info.plist" 2>/dev/null || true; \
		plutil -replace CFBundleDisplayName -string "$(APP_NAME)" "$(MACOS_DIR)/$(APP_NAME).app/Contents/Info.plist" 2>/dev/null || true; \
		plutil -replace CFBundleIdentifier -string "$$BUNDLE_ID" "$(MACOS_DIR)/$(APP_NAME).app/Contents/Info.plist" 2>/dev/null || true; \
		plutil -replace CFBundleIconFile -string "DUCAD.icns" "$(MACOS_DIR)/$(APP_NAME).app/Contents/Info.plist" 2>/dev/null || true; \
	fi
	@if [ -f "assets/DUCAD.icns" ]; then \
		mkdir -p "$(MACOS_DIR)/$(APP_NAME).app/Contents/Resources"; \
		cp "assets/DUCAD.icns" "$(MACOS_DIR)/$(APP_NAME).app/Contents/Resources/"; \
	fi
	@if [ -f "$(MACOS_DIR)/$(APP_NAME).app/Contents/MacOS/$(BIN_NAME)" ]; then \
		chmod 755 "$(MACOS_DIR)/$(APP_NAME).app/Contents/MacOS/$(BIN_NAME)"; \
	fi
	@if [ -f "$(MACOS_DIR)/$(APP_NAME).app/Contents/MacOS/$(APP_NAME)" ]; then \
		chmod 755 "$(MACOS_DIR)/$(APP_NAME).app/Contents/MacOS/$(APP_NAME)"; \
	fi
	xattr -cr "$(MACOS_DIR)/$(APP_NAME).app" 2>/dev/null || true
	find "$(MACOS_DIR)/$(APP_NAME).app" -name ".DS_Store" -delete 2>/dev/null || true
	@if [ -n "$$APPLE_APP_IDENTITY" ]; then \
		echo "🔏 Codesign with App Store entitlements (binary + app)..."; \
		codesign --force --timestamp --options runtime --entitlements macos/DUCAD.entitlements --keychain "$$HOME/Library/Keychains/login.keychain-db" -s "$$APPLE_APP_IDENTITY" "$$(find $(MACOS_DIR)/$(APP_NAME).app/Contents/MacOS -type f | head -n1)" 2>/dev/null || true; \
		codesign --force --timestamp --options runtime --entitlements macos/DUCAD.entitlements --keychain "$$HOME/Library/Keychains/login.keychain-db" -s "$$APPLE_APP_IDENTITY" -v "$(MACOS_DIR)/$(APP_NAME).app"; \
	elif [ -n "$$APPLE_IDENTITY" ]; then \
		echo "🔏 Codesign with Developer ID entitlements (binary + app)..."; \
		codesign --force --timestamp --options runtime --entitlements macos/DUCAD-DeveloperID.entitlements --keychain "$$HOME/Library/Keychains/login.keychain-db" -s "$$APPLE_IDENTITY" "$$(find $(MACOS_DIR)/$(APP_NAME).app/Contents/MacOS -type f | head -n1)" 2>/dev/null || true; \
		codesign --force --timestamp --options runtime --entitlements macos/DUCAD-DeveloperID.entitlements --keychain "$$HOME/Library/Keychains/login.keychain-db" -s "$$APPLE_IDENTITY" -v "$(MACOS_DIR)/$(APP_NAME).app"; \
	else \
		echo "⚠️  Skipping codesign (set APPLE_IDENTITY or APPLE_APP_IDENTITY)."; \
	fi
	@if [ -n "$$APPLE_APP_IDENTITY" ] || [ -n "$$APPLE_IDENTITY" ]; then \
		codesign --verify --deep --strict --verbose=2 "$(MACOS_DIR)/$(APP_NAME).app" 2>/dev/null || true; \
	fi
	@if command -v hdiutil >/dev/null 2>&1; then \
		echo "💿 Create DMG"; \
		rm -f "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).dmg"; \
		DMG_SIZE=$$(( $$(du -sm $(MACOS_DIR)/$(APP_NAME).app | cut -f1) + 100 )); \
		hdiutil create -volname "$(APP_NAME)" -srcfolder "$(MACOS_DIR)/$(APP_NAME).app" -ov -format UDZO -size $${DMG_SIZE}m "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).dmg"; \
		if [ -n "$$APPLE_IDENTITY" ]; then \
			echo "🔏 Sign DMG with Developer ID"; \
			codesign --force --timestamp --keychain "$$HOME/Library/Keychains/login.keychain-db" --sign "$$APPLE_IDENTITY" "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).dmg"; \
		fi; \
	fi
	@if [ -n "$$NOTARIZE" ] && [ -n "$$APPLE_ID" ] && [ -n "$$APPLE_PASSWORD" ] && [ -n "$$APPLE_TEAM_ID" ]; then \
		echo "📤 Notarize app bundle"; \
		ditto -c -k --keepParent "$(MACOS_DIR)/$(APP_NAME).app" "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).zip"; \
		if xcrun notarytool submit "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).zip" --apple-id "$$APPLE_ID" --team-id "$$APPLE_TEAM_ID" --password "$$APPLE_PASSWORD" --wait; then \
			echo "✅ App notarized, stapling..."; \
			xcrun stapler staple "$(MACOS_DIR)/$(APP_NAME).app"; \
			echo "📤 Notarize DMG"; \
			if [ -f "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).dmg" ]; then \
				if xcrun notarytool submit "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).dmg" --apple-id "$$APPLE_ID" --team-id "$$APPLE_TEAM_ID" --password "$$APPLE_PASSWORD" --wait; then \
					echo "✅ DMG notarized, stapling..."; \
					xcrun stapler staple "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).dmg"; \
				else \
					echo "❌ DMG notarization failed"; \
				fi; \
			fi; \
		else \
			echo "❌ App notarization failed"; \
		fi; \
		rm -f "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).zip"; \
	else \
		echo "ℹ️  Notarization skipped (set NOTARIZE=1, APPLE_ID, APPLE_PASSWORD, APPLE_TEAM_ID)"; \
	fi
	@echo "✅ macOS bundle done."

pkg-macos-store: bundle-macos
	@echo "📦 Build signed .pkg (App Store / distribution)"
	@if [ -z "$$APPLE_IDENTITY_INS" ] && [ -z "$$APPLE_IDENTITY" ]; then echo "❌ APPLE_IDENTITY_INS or APPLE_IDENTITY required (Installer identity)"; exit 1; fi
	@APPLE_BUNDLE_ID=$${APPLE_BUNDLE_ID:-id.jayuda.ducad}; \
	INS_IDENTITY="$${APPLE_IDENTITY_INS:-$$APPLE_IDENTITY}"; \
	APP_PATH="$(MACOS_DIR)/$(APP_NAME).app"; \
	APP_IDENTITY="$${APPLE_APP_IDENTITY:-3rd Party Mac Developer Application: PT. VNEU TEKNOLOGI INDONESIA (YD4J5Z6A4G)}"; \
	if [ -n "$$PROVISIONING_PROFILE" ] && [ -f "$$PROVISIONING_PROFILE" ]; then \
		echo "🔗 Embed provisioning profile"; cp "$$PROVISIONING_PROFILE" $$APP_PATH/Contents/embedded.provisionprofile; \
	else echo "ℹ️  No provisioning profile (set PROVISIONING_PROFILE)"; fi; \
	xattr -cr $$APP_PATH 2>/dev/null || true; \
	find $$APP_PATH -name ".DS_Store" -delete 2>/dev/null || true; \
	echo "🔏 Re-codesign with Mac App Store entitlements"; \
	codesign --force --timestamp --options runtime --entitlements macos/DUCAD.entitlements --keychain "$$HOME/Library/Keychains/login.keychain-db" -s "$$APP_IDENTITY" "$$(find $$APP_PATH/Contents/MacOS -type f | head -n1)" 2>/dev/null || true; \
	codesign --force --timestamp --options runtime --entitlements macos/DUCAD.entitlements --keychain "$$HOME/Library/Keychains/login.keychain-db" -s "$$APP_IDENTITY" -v $$APP_PATH; \
	echo "📦 Create installer package"; \
	productbuild --component $$APP_PATH /Applications "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).pkg" --sign "$$INS_IDENTITY" --identifier "$$APPLE_BUNDLE_ID"; \
	if [ -n "$$NOTARIZE" ] && [ -n "$$APPLE_ID" ] && [ -n "$$APPLE_PASSWORD" ] && [ -n "$$APPLE_TEAM_ID" ]; then \
		xcrun notarytool submit "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).pkg" --apple-id "$$APPLE_ID" --team-id "$$APPLE_TEAM_ID" --password "$$APPLE_PASSWORD" --wait && xcrun stapler staple "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).pkg" || true; \
	else echo "ℹ️  Notarization skipped for pkg"; fi
	@echo "✅ pkg created at $(MACOS_DIR)/$(APP_NAME)-$(VERSION).pkg"

build-linux: create-dirs
	@echo "🐧 Build Linux"
	cargo build --release -p $(PKG_NAME) --target $(LINUX_X86_TARGET) || cargo build --release -p $(PKG_NAME)
	@if [ -f $(BUILD_DIR)/$(LINUX_X86_TARGET)/release/$(BIN_NAME) ]; then \
		cp $(BUILD_DIR)/$(LINUX_X86_TARGET)/release/$(BIN_NAME) $(LINUX_DIR)/$(BIN_NAME)-x86_64; \
	elif [ -f $(BUILD_DIR)/release/$(BIN_NAME) ]; then \
		cp $(BUILD_DIR)/release/$(BIN_NAME) $(LINUX_DIR)/$(BIN_NAME)-x86_64; \
	fi
	@echo "✅ Linux builds ready."

bundle-linux: build-linux
	@echo "📦 Package Linux"
	cd $(LINUX_DIR) && tar -czf ducad-$(VERSION)-linux-x86_64.tar.gz $(BIN_NAME)-x86_64 2>/dev/null || true
	@echo "✅ Linux packaging done."

build-windows: create-dirs
	@echo "🪟 Build Windows"
	cargo build --release -p $(PKG_NAME) --target $(WINDOWS_X86_TARGET) || true
	@if [ -f $(BUILD_DIR)/$(WINDOWS_X86_TARGET)/release/$(BIN_NAME).exe ]; then \
		cp $(BUILD_DIR)/$(WINDOWS_X86_TARGET)/release/$(BIN_NAME).exe $(WINDOWS_DIR)/$(BIN_NAME)-x86_64.exe; \
	fi
	@echo "✅ Windows builds ready."

bundle-windows: build-windows
	@echo "🪟 Package Windows"
	@if [ -f $(WINDOWS_DIR)/$(BIN_NAME)-x86_64.exe ]; then \
		cd $(WINDOWS_DIR) && zip -r ducad-$(VERSION)-windows-x86_64.zip $(BIN_NAME)-x86_64.exe; \
	fi
	@echo "✅ Windows packaging done."

build-ipad: create-dirs
	@echo "📱 Building iPadOS target..."
	cargo build --release -p $(PKG_NAME) --target $(IOS_ARM_TARGET)
	@mkdir -p $(IOS_DIR)
	@if [ -f $(BUILD_DIR)/$(IOS_ARM_TARGET)/release/$(BIN_NAME) ]; then \
		cp $(BUILD_DIR)/$(IOS_ARM_TARGET)/release/$(BIN_NAME) $(IOS_DIR)/$(BIN_NAME)-ios-arm64; \
	fi
	@echo "✅ iPadOS binary build completed."

archive-ipad: build-ipad
	@echo "📦 Archiving DUCAD for iPadOS..."
	@echo "ℹ️  Template Info.plist available at crates/ducad-app/ios/Info.plist.template"

ipa-ipad: archive-ipad
	@echo "📦 Exporting signed .ipa for iPad..."

publish-ipad: ipa-ipad
	@echo "🚀 Publishing DUCAD iPad app to App Store Connect / TestFlight..."

release: clean install-deps bundle-macos
	@echo "🎉 Release completed"
	@echo "📱 macOS artifacts:"
	@find $(MACOS_DIR) -maxdepth 1 -name '*.dmg' -o -name '*.pkg' -o -name '*.app' 2>/dev/null | sed 's/^/  /' || true

build:
	cargo build --release -p $(PKG_NAME)

run:
	cargo run -p $(PKG_NAME)

dev:
	cargo build -p $(PKG_NAME)

test:
	cargo test

check:
	cargo check -p $(PKG_NAME)

fmt:
	cargo fmt

info:
	@echo "Name: $(APP_NAME)"
	@echo "Version: $(VERSION)"
	@echo "Package: $(PKG_NAME)"
	@echo "Binary: $(BIN_NAME)"
	@echo "Dist dir: $(DIST_DIR)"

notarize:
	@echo "🔐 Notarizing $(APP_NAME) v$(VERSION)"
	@if [ -z "$$APPLE_ID" ] || [ -z "$$APPLE_TEAM_ID" ] || [ -z "$$APPLE_PASSWORD" ]; then \
		echo "❌ Missing environment variables. Please set:"; \
		echo "export APPLE_ID='nunung.pamungkas@vneu.co.id'"; \
		echo "export APPLE_TEAM_ID='YD4J5Z6A4G'"; \
		echo "export APPLE_PASSWORD='your-app-specific-password'"; \
		exit 1; \
	fi
	@if [ -d "$(MACOS_DIR)/$(APP_NAME).app" ]; then \
		echo "📱 Notarizing app bundle..."; \
		ditto -c -k --keepParent "$(MACOS_DIR)/$(APP_NAME).app" "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).zip"; \
		if xcrun notarytool submit "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).zip" --apple-id "$$APPLE_ID" --team-id "$$APPLE_TEAM_ID" --password "$$APPLE_PASSWORD" --wait; then \
			echo "✅ App notarized, stapling..."; \
			xcrun stapler staple "$(MACOS_DIR)/$(APP_NAME).app"; \
		else \
			echo "❌ App notarization failed"; \
		fi; \
		rm -f "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).zip"; \
	fi
	@if [ -f "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).dmg" ]; then \
		echo "💿 Notarizing DMG..."; \
		if xcrun notarytool submit "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).dmg" --apple-id "$$APPLE_ID" --team-id "$$APPLE_TEAM_ID" --password "$$APPLE_PASSWORD" --wait; then \
			echo "✅ DMG notarized, stapling..."; \
			xcrun stapler staple "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).dmg"; \
		else \
			echo "❌ DMG notarization failed"; \
		fi; \
	fi
	@if [ -f "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).pkg" ]; then \
		echo "📦 Notarizing PKG..."; \
		if xcrun notarytool submit "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).pkg" --apple-id "$$APPLE_ID" --team-id "$$APPLE_TEAM_ID" --password "$$APPLE_PASSWORD" --wait; then \
			echo "✅ PKG notarized, stapling..."; \
			xcrun stapler staple "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).pkg"; \
		else \
			echo "❌ PKG notarization failed"; \
		fi; \
	fi
	@echo "🎉 Notarization completed!"

notarize-check:
	@echo "🔍 Checking notarization status for $(APP_NAME) v$(VERSION)"
	@if [ -d "$(MACOS_DIR)/$(APP_NAME).app" ]; then \
		echo "📱 App Gatekeeper status:"; \
		spctl -a -t exec -v "$(MACOS_DIR)/$(APP_NAME).app" 2>&1 && echo "✅ App accepted" || echo "❌ App rejected"; \
	fi
	@if [ -f "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).dmg" ]; then \
		echo "💿 DMG Gatekeeper status:"; \
		spctl -a -t open --context context:primary-signature -v "$(MACOS_DIR)/$(APP_NAME)-$(VERSION).dmg" 2>&1 && echo "✅ DMG accepted" || echo "❌ DMG rejected"; \
	fi

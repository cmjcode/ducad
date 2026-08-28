TARGETS = all help install-deps clean build-macos bundle-macos pkg-macos-store \
          build-ipad archive-ipad ipa-ipad publish-ipad \
          build-linux bundle-linux build-windows bundle-windows \
          release build run dev test check fmt info notarize notarize-check

.PHONY: $(TARGETS) default

default:
	@$(MAKE) -C ducad-editor help

$(TARGETS):
	@$(MAKE) -C ducad-editor $@

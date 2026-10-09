TARGETS = all help install-deps clean clean-no-occt build-macos bundle-macos pkg-macos-store \
          build-ipad archive-ipad ipa-ipad publish-ipad publish-all publish-apple-all \
          build-linux bundle-linux build-windows bundle-windows \
          release build run dev test check fmt info notarize notarize-check

.PHONY: $(TARGETS) default

default:
	@$(MAKE) -C ducad-editor help

$(TARGETS):
	@$(MAKE) -C ducad-editor $@

# Publikasi paket AUR `ducad` (sumber dari tag v$(VERSION) di GitHub).
# Lihat scripts/update-aur.sh; pakai `make publish-aur ARGS=--no-push` untuk uji.
.PHONY: publish-aur
publish-aur:
	@scripts/update-aur.sh ducad $(ARGS)

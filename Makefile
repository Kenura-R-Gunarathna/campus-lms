# AUR Folders
AUR_GIT_DIR ?= $(HOME)/aur/campus-lms-git
AUR_BIN_DIR ?= $(HOME)/aur/campus-lms-bin

# AUR URLs
AUR_GIT_URL ?= https://aur.archlinux.org/campus-lms-git.git
AUR_BIN_URL ?= ssh://aur@aur.archlinux.org/campus-lms-bin.git

.PHONY: help
help:
	@echo "Available commands:"
	@echo "  make aur-init         - Set up local folders for both AUR packages (-git and -bin)"
	@echo "  make aur-push-git     - Sync and push source package to AUR"
	@echo "  make aur-push-bin v=X.Y.Z sha=HASH"
	@echo "                        - Sync and push pre-compiled binary package to AUR"
	@echo "                          (Note: Get the SHA from GitHub Release assets)"
	@echo "  make push-all         - Push code to GitHub and update -git AUR"
	@echo "  make release v=X.Y.Z  - Create tag and trigger GitHub binary build"

.PHONY: aur-init
aur-init:
	@mkdir -p $(shell dirname $(AUR_GIT_DIR))
	@if [ ! -d "$(AUR_GIT_DIR)" ]; then git clone $(AUR_GIT_URL) $(AUR_GIT_DIR); fi
	@if [ ! -d "$(AUR_BIN_DIR)" ]; then git clone $(AUR_BIN_URL) $(AUR_BIN_DIR); fi

.PHONY: aur-push-git
aur-push-git:
	@echo "Updating campus-lms-git..."
	makepkg --printsrcinfo > .SRCINFO
	cp PKGBUILD .SRCINFO campus-lms.desktop campus-lms-daemon.service $(AUR_GIT_DIR)/
	cd $(AUR_GIT_DIR) && git add . && (git diff --cached --quiet || git commit -m "Update source") && git push origin master

.PHONY: aur-push-bin
aur-push-bin:
	@if [ -z "$(v)" ] || [ -z "$(sha)" ]; then \
		echo "Error: Need version and sha, e.g., make aur-push-bin v=0.2.2 sha=abc123..."; exit 1; \
	fi
	@echo "Updating campus-lms-bin to v$(v)..."
	# Update the binary PKGBUILD with new version and SHA
	sed -e "s/pkgver=.*/pkgver=$(v)/" \
	    -e "s/sha256sums=(.*/sha256sums=(/" \
	    -e "/sha256sums=(/!b;n;c\    '$(sha)'" \
	    PKGBUILD-bin > $(AUR_BIN_DIR)/PKGBUILD
	cp campus-lms.desktop campus-lms-daemon.service $(AUR_BIN_DIR)/
	cp assets/icon.png $(AUR_BIN_DIR)/campus-lms.png
	cd $(AUR_BIN_DIR) && makepkg --printsrcinfo > .SRCINFO && \
	git add . && (git diff --cached --quiet || git commit -m "Release v$(v)") && git push origin master

.PHONY: push-all
push-all:
	git add . && git commit -m "Update" && git push origin main
	$(MAKE) aur-push-git

.PHONY: release
release:
	@if [ -z "$(v)" ]; then echo "Error: Provide a version, e.g., make release v=1.0.0"; exit 1; fi
	git tag -a v$(v) -m "Release v$(v)"
	git push origin v$(v)
	@echo ""
	@echo "Tag pushed. GitHub Actions will now:"
	@echo "  1. Build Linux + Windows binaries"
	@echo "  2. Create GitHub Release with binaries"
	@echo "  3. Auto-publish campus-lms-bin to AUR (with correct SHA)"
	@echo "  4. Auto-publish campus-lms-git to AUR"
	@echo ""
	@echo "Watch progress: https://github.com/Kenura-R-Gunarathna/campus-lms/actions"
	@echo ""
	@echo "Manual fallback (only if CI fails):"
	@echo "  make aur-push-bin v=$(v) sha=PASTE_SHA_HERE"

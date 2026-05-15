# AUR Update Configuration
AUR_DIR ?= $(HOME)/aur/campus-lms-git
AUR_REPO_URL ?= https://aur.archlinux.org/campus-lms-git.git

.PHONY: help
help:
	@echo "Available commands:"
	@echo "  make aur-init    - Clone the AUR repository for the first time"
	@echo "  make aur-sync    - Update .SRCINFO and copy files to $(AUR_DIR)"
	@echo "  make aur-push    - Sync and push changes to AUR"
	@echo "  make push-all    - Push to GitHub and then update AUR"
	@echo "  make release v=X.Y.Z - Create a new tag and trigger GitHub Release"

.PHONY: release
release:
	@if [ -z "$(v)" ]; then echo "Error: Provide a version, e.g., make release v=1.0.0"; exit 1; fi
	@echo "Tagging version v$(v)..."
	git tag -a v$(v) -m "Release v$(v)"
	@echo "Pushing tag to GitHub..."
	git push origin v$(v)
	@echo "GitHub Action will now build binaries for Linux and Windows."

.PHONY: aur-init
aur-init:
	@echo "Ensuring AUR parent directory exists..."
	mkdir -p $(shell dirname $(AUR_DIR))
	@if [ ! -d "$(AUR_DIR)" ]; then \
		echo "Cloning AUR repository from $(AUR_REPO_URL)..."; \
		git clone $(AUR_REPO_URL) $(AUR_DIR); \
	else \
		echo "AUR directory already exists at $(AUR_DIR)"; \
	fi

.PHONY: aur-sync
aur-sync:
	@echo "Generating .SRCINFO..."
	makepkg --printsrcinfo > .SRCINFO
	@echo "Copying files to AUR directory..."
	cp PKGBUILD .SRCINFO campus-lms.desktop campus-lms-daemon.service $(AUR_DIR)/

.PHONY: aur-push
aur-push: aur-sync
	@echo "Committing and pushing to AUR..."
	cd $(AUR_DIR) && \
	git add PKGBUILD .SRCINFO campus-lms.desktop campus-lms-daemon.service && \
	(git diff --cached --quiet || git commit -m "Update to latest version") && \
	git push origin master

.PHONY: push-all
push-all:
	@echo "Pushing main repository to GitHub..."
	git add . && git commit -m "Update" && git push origin main
	$(MAKE) aur-push

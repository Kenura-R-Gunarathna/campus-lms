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
	git commit -m "Update to latest version" && \
	git push origin master

.PHONY: push-all
push-all:
	@echo "Pushing main repository to GitHub..."
	git add . && git commit -m "Update" && git push origin main
	$(MAKE) aur-push

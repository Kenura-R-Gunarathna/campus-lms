# Campus LMS

Desktop Moodle client for University of Colombo Faculty of Science, built with Rust + egui.

## Features

- Browse enrolled courses and content
- Download files, stream video/audio
- Assignment submission with file upload
- Diff-based content change tracking — see exactly what changed since your last visit
- Diff history screen with timeline graph and snapshot comparison
- Desktop notifications for new Moodle notifications and course content updates
- Background daemon for notifications when the app is closed
- Grades, calendar, announcements
- Local SQLite storage — fast, offline-capable

## Install

### Arch Linux (AUR)

Pre-compiled binary (faster install, recommended):
```bash
paru -S campus-lms-bin
# or
yay -S campus-lms-bin
```

Build from source (tracks `main`):
```bash
paru -S campus-lms-git
# or
yay -S campus-lms-git
```

### Windows

Download `campus-lms-windows-x86_64.exe` from the [latest release](https://github.com/Kenura-R-Gunarathna/campus-lms/releases/latest) and run.

### Other Linux (build from source)

Requires: `rust`, `cargo`, and a C compiler.

```bash
git clone https://github.com/Kenura-R-Gunarathna/campus-lms
cd campus-lms
cargo build --release
./target/release/campus-lms
```

Runtime dependencies: `libxkbcommon`, `libgl`, `dbus`, `libsecret`, `openssl`, `wayland`

On Fedora: `sudo dnf install libxkbcommon mesa-libGL dbus-devel libsecret-devel openssl-devel`

On Ubuntu: `sudo apt install libxkbcommon-dev libgl1 libdbus-1-dev libsecret-1-dev libssl-dev`

## Background Notifications

The app polls Moodle every 10 minutes for new notifications and every 30 minutes for course content changes, sending desktop notifications.

**Option 1 — In-app toggle (AUR install):**
Open Settings → enable "Run notification daemon on login".

**Option 2 — systemd user service (AUR install):**
```bash
systemctl --user enable --now campus-lms-daemon
```

**Option 3 — manual (source build):**
```bash
cargo build --release
cp target/release/campus-lms ~/.local/bin/campus-lms
# then enable in Settings or run: campus-lms --background
```

> Notifications require a desktop notification daemon: `dunst` (X11) or `mako` (Wayland).

## Diff Tracking

Every time you open a course, the app compares current content against stored fingerprints and records what changed. The "What's New" panel shows recent changes with click-to-expand diffs. The full diff history screen shows a timeline of all snapshots with side-by-side comparison.

## Updates

The app checks GitHub for a newer release at most once every 24 hours. When a new version is found, a banner appears at the top of the window:

- **AUR / system-installed**: banner suggests `yay -Syu` (or your package manager) to update.
- **Standalone build**: banner links to the GitHub release for manual download.

## Releasing (maintainers)

Tag a release; CI builds binaries and auto-publishes both AUR packages:

```bash
make release v=X.Y.Z
```

This triggers `.github/workflows/release.yml` which:
1. Builds Linux + Windows binaries via Cargo
2. Creates the GitHub Release with both binaries attached
3. Pushes updated PKGBUILDs + `.SRCINFO` to `campus-lms-bin` and `campus-lms-git` on AUR (SHA256 computed from the freshly built Linux binary)

Required GitHub secret: `AUR_SSH_KEY` (private SSH key whose public key is registered on the AUR account).

## License

MIT

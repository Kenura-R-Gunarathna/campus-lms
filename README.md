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

```bash
paru -S campus-lms-git
# or
yay -S campus-lms-git
```

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

## License

MIT

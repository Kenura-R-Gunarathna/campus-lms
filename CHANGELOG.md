# Changelog

All notable changes to Campus LMS are tracked here.
Format: `[version] - date — description`

---

## [0.3.4] - 2026-05-16
### Added
- App icon (window + system) — installed under `/usr/share/icons/hicolor/512x512/apps/campus-lms.png` for AUR users
- `campus-lms-bin` AUR package — pre-compiled binary install path (no Rust toolchain required)
- Install-method detection — update banner now redirects AUR/system-managed installs to their package manager instead of suggesting an in-app overwrite
- 24-hour cache on the GitHub-Releases update check (was hitting the API every app start)
- Auto-publish release pipeline — `make release v=X.Y.Z` tags + pushes, GitHub Actions builds Linux/Windows binaries and pushes both AUR packages (`-bin` with SHA, `-git` with `.SRCINFO`) via SSH
- Windows release binary attached to GitHub releases

### Changed
- Replaced Unicode arrow glyphs (▲ ▼) with `egui_phosphor::regular::CARET_UP/DOWN` so they render on all systems
- `Cargo.toml` version field now tracks release tags (was stuck at 0.1.0)
- `.desktop` `StartupWMClass=campus-lms` now matched by the window — set via `ViewportBuilder::with_app_id`, so launchers no longer show two taskbar entries

### Fixed
- AUR `campus-lms-bin` package now exists on the AUR (was only referenced in repo; never submitted)

---

## [0.3.0] - 2026-05-03
### Added
- Persistent login via OS keyring (GNOME Keyring / KWallet)
  - Password stored encrypted at OS level on first login
  - Token validated on startup; silent re-auth if expired
  - Login screen only shown if keyring is missing or Logout clicked
- Multi-page tab navigation: Courses / Assignments / Grades
- User fullname displayed in top-right of tab bar
- Logout button in tab bar

### Changed
- Increased font sizes across courses screen (15px names, 14px tabs)
- HTML entity decoding in course names (e.g. `&amp;` → `&`)
- Fallback category label "Uncategorised" for courses with no category

---

## [0.2.0] - 2026-05-03
### Added
- Course list screen with category sidebar filter
- Search bar for filtering courses by name or code
- Session restore on app startup (token + userid from SQLite)
- User fullname persisted across restarts

### Changed
- Login screen redesigned: centred card layout, Enter key submits
- Logout clears SQLite session

---

## [0.1.0] - 2026-05-03
### Added
- Project scaffold: egui 0.29 + eframe (glow/OpenGL backend)
- Moodle Web Services API client (`login/token.php`, `core_webservice_get_site_info`, `core_enrol_get_users_courses`)
- SQLite storage for session data via rusqlite
- Async HTTP via reqwest + tokio
- Login screen with email/password fields
- Token-based auth (moodle_mobile_app service)



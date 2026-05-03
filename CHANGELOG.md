# Changelog

All notable changes to Campus LMS are tracked here.
Format: `[version] - date — description`

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

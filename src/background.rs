use crate::api::MoodleClient;
use crate::storage::Storage;

const NOTIF_POLL_SECS: u64 = 600;   // 10 min
const CONTENT_POLL_SECS: u64 = 1800; // 30 min

pub fn desktop_notify(title: &str, body: &str) {
    let _ = notify_rust::Notification::new()
        .summary(title)
        .body(body)
        .icon("dialog-information")
        .timeout(notify_rust::Timeout::Milliseconds(6000))
        .show();
}

/// No-GUI daemon mode (`--background` flag)
pub async fn run_daemon() {
    let storage = match Storage::open() {
        Ok(s) => s,
        Err(e) => { eprintln!("storage: {e}"); return; }
    };
    let token    = storage.get("token").ok().flatten().unwrap_or_default();
    let userid: u64 = storage.get("userid").ok().flatten()
        .and_then(|s| s.parse().ok()).unwrap_or(0);
    if token.is_empty() || userid == 0 {
        eprintln!("No session. Log in via GUI first."); return;
    }

    let mut last_notif_id: u64 = storage.get("last_notif_id").ok().flatten()
        .and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut last_content_poll: std::time::Instant =
        std::time::Instant::now() - std::time::Duration::from_secs(CONTENT_POLL_SECS);

    loop {
        // Notifications
        last_notif_id = poll_notifications(&token, userid, last_notif_id, &storage).await;

        // Course content changes (every 30 min)
        if last_content_poll.elapsed().as_secs() >= CONTENT_POLL_SECS {
            poll_content_changes(&token, &storage).await;
            last_content_poll = std::time::Instant::now();
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(NOTIF_POLL_SECS)).await;
    }
}

/// In-app background poller (tokio task)
pub fn spawn_poller(token: String, userid: u64, tx: std::sync::mpsc::Sender<u64>) {
    tokio::spawn(async move {
        let mut last_id: u64 = 0;
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(NOTIF_POLL_SECS)).await;
            let new_count = count_new_notifs(&token, userid, last_id).await;
            if let Some((count, newest_id)) = new_count {
                last_id = newest_id;
                if count > 0 { let _ = tx.send(count); }
            }
        }
    });
}

async fn count_new_notifs(token: &str, userid: u64, last_id: u64) -> Option<(u64, u64)> {
    let client = MoodleClient::new(token.to_string());
    let notifs = client.notifications(userid, 0).await.ok()?;
    let new: Vec<_> = notifs.notifications.iter()
        .filter(|n| n.id > last_id && !n.is_read).collect();
    let newest_id = notifs.notifications.iter().map(|n| n.id).max().unwrap_or(last_id);
    let subjects: Vec<String> = new.iter().take(3).map(|n| n.subject.clone()).collect();
    let extra = new.len().saturating_sub(3);
    tokio::task::spawn_blocking(move || {
        for s in &subjects { desktop_notify("Campus LMS", s); }
        if extra > 0 { desktop_notify("Campus LMS", &format!("{extra} more new notifications")); }
    }).await.ok();
    Some((new.len() as u64, newest_id))
}

async fn poll_notifications(token: &str, userid: u64, last_id: u64, storage: &Storage) -> u64 {
    if let Some((_count, newest)) = count_new_notifs(token, userid, last_id).await {
        storage.set("last_notif_id", &newest.to_string()).ok();
        return newest;
    }
    last_id
}

/// Poll enrolled courses for content changes, send desktop notification if changes found.
async fn poll_content_changes(token: &str, storage: &Storage) {
    // Load enrolled courses from cache
    let courses_json = match storage.load_cache("courses") {
        Ok(Some(j)) => j,
        _ => return,
    };
    let courses: Vec<crate::api::types::Course> = match serde_json::from_str(&courses_json) {
        Ok(c) => c,
        Err(_) => return,
    };

    let client = MoodleClient::new(token.to_string());
    let mut changed_courses: Vec<String> = vec![];
    let mut total_changes = 0usize;

    for course in &courses {
        let sections = match client.course_contents(course.id).await {
            Ok(s) => s,
            Err(_) => continue,
        };

        let stored_mods = storage.load_fingerprints(course.id).unwrap_or_default();
        let stored_secs = storage.load_section_fingerprints(course.id).unwrap_or_default();

        let (changes, new_mods, new_secs, rem_mods, rem_secs) = 
            crate::telemetry::diff_content(course.id, &sections, &stored_mods, &stored_secs);

        if !changes.is_empty() {
            total_changes += changes.len();
            changed_courses.push(course.shortname.clone());
            let _ = storage.save_changes(&changes);
        }

        let _ = storage.upsert_fingerprints(course.id, &new_mods);
        let _ = storage.upsert_section_fingerprints(course.id, &new_secs);
        let _ = storage.delete_fingerprints(&rem_mods);
        let _ = storage.delete_section_fingerprints(&rem_secs);

        // Small delay to avoid hammering the server
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    if !changed_courses.is_empty() {
        let course_list = changed_courses.join(", ");
        let body = format!(
            "{total_changes} change{} in: {course_list}",
            if total_changes == 1 { "" } else { "s" }
        );
        tokio::task::spawn_blocking(move || {
            desktop_notify("Campus LMS — Content Updated", &body);
        }).await.ok();
    }
}

pub fn is_dev_binary() -> bool {
    std::env::current_exe().map(|p| {
        let s = p.to_string_lossy();
        s.contains("/target/debug/") || s.contains("/tmp/")
            || (s.contains("/target/release/") && s.contains("/deps/"))
    }).unwrap_or(false)
}

pub struct DaemonStatus {
    pub desktop_file_exists: bool,
    pub desktop_exe_path: Option<String>,
    pub current_exe_path: String,
    pub paths_match: bool,
    pub is_dev_binary: bool,
}

pub fn daemon_status() -> DaemonStatus {
    let current_exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let desktop_path = dirs_next::config_dir()
        .map(|d| d.join("autostart").join("campus-lms.desktop"));
    let desktop_file_exists = desktop_path.as_ref().map(|p| p.exists()).unwrap_or(false);
    let desktop_exe_path = desktop_path
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|contents| {
            contents.lines()
                .find(|l| l.starts_with("Exec="))
                .map(|l| l["Exec=".len()..].trim().trim_end_matches(" --background").to_string())
        });
    let paths_match = desktop_exe_path.as_deref() == Some(current_exe.as_str());
    DaemonStatus {
        desktop_file_exists,
        paths_match,
        desktop_exe_path,
        current_exe_path: current_exe,
        is_dev_binary: is_dev_binary(),
    }
}

pub fn create_autostart() -> std::io::Result<()> {
    if is_dev_binary() {
        return Err(std::io::Error::other(
            "Dev binary detected. Build with `cargo build --release` and install \
             to a stable path (e.g. ~/.local/bin/campus-lms) first."
        ));
    }
    let exe = std::env::current_exe()?;
    let dir = dirs_next::config_dir()
        .ok_or_else(|| std::io::Error::other("no config dir"))?
        .join("autostart");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("campus-lms.desktop"), format!(
        "[Desktop Entry]\nType=Application\nName=Campus LMS Notifications\nExec={} --background\nHidden=false\nX-GNOME-Autostart-enabled=true\n",
        exe.display()
    ))
}

pub fn remove_autostart() {
    if let Some(dir) = dirs_next::config_dir() {
        let _ = std::fs::remove_file(dir.join("autostart").join("campus-lms.desktop"));
    }
}

pub fn autostart_enabled() -> bool {
    dirs_next::config_dir()
        .map(|d| d.join("autostart").join("campus-lms.desktop").exists())
        .unwrap_or(false)
}

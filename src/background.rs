use crate::api::MoodleClient;
use crate::storage::Storage;

const POLL_SECS: u64 = 600;

fn desktop_notify(title: &str, body: &str) {
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
    let mut last_id: u64 = storage.get("last_notif_id").ok().flatten()
        .and_then(|s| s.parse().ok()).unwrap_or(0);
    loop {
        last_id = poll_once(&token, userid, last_id, Some(&storage)).await;
        tokio::time::sleep(tokio::time::Duration::from_secs(POLL_SECS)).await;
    }
}

/// In-app background poller (tokio task — no Storage held across awaits)
pub fn spawn_poller(token: String, userid: u64, tx: std::sync::mpsc::Sender<u64>) {
    tokio::spawn(async move {
        let mut last_id: u64 = 0;
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(POLL_SECS)).await;
            let new_count = count_new_notifs(&token, userid, last_id).await;
            if let Some((count, newest_id)) = new_count {
                last_id = newest_id;
                if count > 0 { let _ = tx.send(count); }
            }
        }
    });
}

/// Poll without holding any non-Send type across awaits
async fn count_new_notifs(token: &str, userid: u64, last_id: u64) -> Option<(u64, u64)> {
    let client = MoodleClient::new(token.to_string());
    let notifs = client.notifications(userid, 0).await.ok()?;
    let new: Vec<_> = notifs.notifications.iter()
        .filter(|n| n.id > last_id && !n.is_read).collect();
    let newest_id = notifs.notifications.iter().map(|n| n.id).max().unwrap_or(last_id);
    // Send desktop notifications (blocking call, moved off async thread)
    let subjects: Vec<String> = new.iter().take(3).map(|n| n.subject.clone()).collect();
    let extra = new.len().saturating_sub(3);
    tokio::task::spawn_blocking(move || {
        for s in &subjects { desktop_notify("Campus LMS", s); }
        if extra > 0 { desktop_notify("Campus LMS", &format!("{extra} more new notifications")); }
    }).await.ok();
    Some((new.len() as u64, newest_id))
}

/// Poll used in daemon mode where Storage can be held (not spawned)
async fn poll_once(token: &str, userid: u64, last_id: u64, storage: Option<&Storage>) -> u64 {
    if let Some(Some((count, newest))) = Some(count_new_notifs(token, userid, last_id).await) {
        if let Some(s) = storage { s.set("last_notif_id", &newest.to_string()).ok(); }
        return newest;
    }
    last_id
}

pub fn create_autostart() -> std::io::Result<()> {
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

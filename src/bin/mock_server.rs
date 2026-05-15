/// Campus LMS mock Moodle server for GUI testing and integration tests.
///
/// Usage:
///   cargo run --bin mock_server
///
/// The server listens on http://127.0.0.1:8888
/// In Settings → Server, click "Use localhost:8888", then login with any username/password.
///
/// State is kept in memory and optionally persisted to /tmp/campus-lms-mock.json.
/// Admin endpoints (no auth required):
///   GET /admin/state        — show current content version
///   GET /admin/bump         — increment content version (simulates instructor update)
///   GET /admin/reset        — reset to version 1
///   GET /admin/notif        — inject a fake notification

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const PORT: u16 = 8888;
const STATE_FILE: &str = "/tmp/campus-lms-mock.json";

#[derive(Clone)]
struct MockState {
    content_version: u32,
    notif_count: u32,
}

impl Default for MockState {
    fn default() -> Self { Self { content_version: 1, notif_count: 0 } }
}

impl MockState {
    fn save(&self) {
        let json = format!(
            "{{\"content_version\":{},\"notif_count\":{}}}",
            self.content_version, self.notif_count
        );
        std::fs::write(STATE_FILE, json).ok();
    }

    fn load() -> Self {
        if let Ok(data) = std::fs::read_to_string(STATE_FILE) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data) {
                return Self {
                    content_version: v["content_version"].as_u64().unwrap_or(1) as u32,
                    notif_count: v["notif_count"].as_u64().unwrap_or(0) as u32,
                };
            }
        }
        Self::default()
    }
}

#[tokio::main]
async fn main() {
    let state = Arc::new(Mutex::new(MockState::load()));
    let listener = TcpListener::bind(format!("127.0.0.1:{PORT}")).await
        .expect("Failed to bind port 8888");

    println!("Campus LMS mock server running on http://127.0.0.1:{PORT}");
    println!("  Admin: GET /admin/state | /admin/bump | /admin/reset | /admin/notif");
    println!("  Login with any username/password");
    println!("  Content version: {}", state.lock().unwrap().content_version);

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                let state = Arc::clone(&state);
                tokio::spawn(async move {
                    if let Err(e) = handle(stream, state).await {
                        eprintln!("{addr}: {e}");
                    }
                });
            }
            Err(e) => eprintln!("Accept error: {e}"),
        }
    }
}

async fn handle(mut stream: TcpStream, state: Arc<Mutex<MockState>>) -> std::io::Result<()> {
    // Read request (up to 32KB)
    let mut buf = vec![0u8; 32768];
    let n = stream.read(&mut buf).await?;
    let raw = String::from_utf8_lossy(&buf[..n]);

    // Parse first line: METHOD PATH HTTP/1.1
    let first_line = raw.lines().next().unwrap_or("");
    let parts: Vec<&str> = first_line.splitn(3, ' ').collect();
    if parts.len() < 2 {
        return respond(&mut stream, 400, "Bad Request", "text/plain", "bad request").await;
    }
    let method = parts[0];
    let full_path = parts[1]; // may include query string

    let (path, query) = match full_path.split_once('?') {
        Some((p, q)) => (p, q),
        None => (full_path, ""),
    };

    // Parse body (for POST with form data)
    let body = if let Some(idx) = raw.find("\r\n\r\n") {
        raw[idx + 4..].to_string()
    } else if let Some(idx) = raw.find("\n\n") {
        raw[idx + 2..].to_string()
    } else {
        String::new()
    };

    // Merge query + body params
    let params = parse_params(if method == "POST" { &body } else { query });

    let response_body = match path {
        "/login/token.php" => handle_login(),
        "/webservice/rest/server.php" => {
            let wsfunction = params.get("wsfunction").map(|s| s.as_str()).unwrap_or("");
            handle_ws(wsfunction, &params, &state)
        }
        "/webservice/upload.php" => handle_upload(),
        "/admin/state" => {
            let s = state.lock().unwrap();
            format!("{{\"content_version\":{},\"notif_count\":{}}}", s.content_version, s.notif_count)
        }
        "/admin/bump" => {
            let mut s = state.lock().unwrap();
            s.content_version += 1;
            let v = s.content_version;
            s.save();
            println!("[admin] Content version bumped to {v}");
            format!("{{\"content_version\":{v},\"message\":\"Content updated. Reopen a course to trigger diff.\"}}")
        }
        "/admin/reset" => {
            let mut s = state.lock().unwrap();
            s.content_version = 1;
            s.notif_count = 0;
            s.save();
            println!("[admin] State reset to version 1");
            "{\"content_version\":1,\"message\":\"Reset to v1.\"}".into()
        }
        "/admin/notif" => {
            let mut s = state.lock().unwrap();
            s.notif_count += 1;
            let n = s.notif_count;
            s.save();
            println!("[admin] Injected notification #{n}");
            format!("{{\"notif_count\":{n},\"message\":\"Notification injected.\"}}")
        }
        _ => {
            eprintln!("404: {method} {path}");
            return respond(&mut stream, 404, "Not Found", "text/plain", "not found").await;
        }
    };

    respond(&mut stream, 200, "OK", "application/json", &response_body).await
}

async fn respond(
    stream: &mut TcpStream,
    status: u16,
    status_text: &str,
    content_type: &str,
    body: &str,
) -> std::io::Result<()> {
    let response = format!(
        "HTTP/1.1 {status} {status_text}\r\n\
         Content-Type: {content_type}; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await
}

fn parse_params(query: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            map.insert(url_decode(k), url_decode(v));
        }
    }
    map
}

fn url_decode(s: &str) -> String {
    let mut out = String::new();
    let bytes = s.replace('+', " ");
    let mut chars = bytes.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h1 = chars.next().unwrap_or('0');
            let h2 = chars.next().unwrap_or('0');
            if let Ok(b) = u8::from_str_radix(&format!("{h1}{h2}"), 16) {
                out.push(b as char);
            }
        } else {
            out.push(c);
        }
    }
    out
}

// ── Response builders ────────────────────────────────────────────────────────

fn handle_login() -> String {
    r#"{"token":"mock-token-abc123","privatetoken":"mock-priv-token"}"#.into()
}

fn handle_upload() -> String {
    r#"[{"itemid":99001,"filename":"test_file.pdf","filesize":1024}]"#.into()
}

fn handle_ws(wsfunction: &str, _params: &HashMap<String, String>, state: &Arc<Mutex<MockState>>) -> String {
    let ver = state.lock().unwrap().content_version;
    let notif_count = state.lock().unwrap().notif_count;

    match wsfunction {
        "core_webservice_get_site_info" => site_info(),
        "tool_mobile_get_autologin_key" => r#"{"autologinurl":"http://localhost:8888/autologin?key=mock"}"#.into(),
        "core_enrol_get_users_courses" => courses(),
        "core_course_get_contents" => course_contents(ver),
        "mod_assign_get_assignments" => r#"{"courses":[]}"#.into(),
        "mod_assign_get_submission_status" => assign_status(),
        "mod_assign_save_submission" => r#"[]"#.into(),
        "mod_assign_submit_for_grading" => r#"[]"#.into(),
        "core_calendar_get_action_events_by_timesort" => r#"{"events":[],"firstid":0,"lastid":0}"#.into(),
        "message_popup_get_popup_notifications" => notifications(notif_count),
        "gradereport_overview_get_course_grades" => r#"{"grades":[]}"#.into(),
        "gradereport_user_get_grade_items" => r#"{"usergrades":[]}"#.into(),
        "core_user_get_users_by_field" => user_profile(),
        "mod_forum_get_forums_by_courses" => r#"[]"#.into(),
        "mod_forum_get_forum_discussions_paginated" => r#"{"discussions":[],"warnings":[]}"#.into(),
        other => {
            eprintln!("[ws] unhandled wsfunction: {other}");
            r#"[]"#.into()
        }
    }
}

fn site_info() -> String {
    r#"{
        "userid": 1,
        "fullname": "Test Student",
        "sitename": "Campus LMS Test Server",
        "siteurl": "http://localhost:8888",
        "token": "mock-token-abc123",
        "userprivateaccesskey": "mock-priv-token",
        "release": "4.1 (Mock)",
        "functions": []
    }"#.into()
}

fn courses() -> String {
    r#"[
        {
            "id": 101,
            "shortname": "CS101",
            "fullname": "Introduction to Computer Science",
            "displayname": "Introduction to Computer Science",
            "enrolledusercount": 50,
            "idnumber": "",
            "visible": 1
        },
        {
            "id": 102,
            "shortname": "MATH201",
            "fullname": "Discrete Mathematics",
            "displayname": "Discrete Mathematics",
            "enrolledusercount": 35,
            "idnumber": "",
            "visible": 1
        }
    ]"#.into()
}

fn course_contents(version: u32) -> String {
    // Each version produces distinct content so every bump triggers a real diff.
    // v1 = baseline; v2+ adds graph traversal module; odd versions use topic A, even use topic B.
    let extra_topic = if version % 2 == 0 {
        "dynamic programming"
    } else if version > 1 {
        "graph traversal"
    } else {
        ""
    };
    let week1_desc = if version == 1 {
        "Introduction to algorithms. Covers basic sorting and searching techniques.".to_string()
    } else {
        format!(
            "Introduction to algorithms (updated v{version}). Covers sorting, searching, and {extra_topic}. \
             Instructor notes revised for clarity."
        )
    };

    let week2_module = if version >= 2 {
        let slide_size = 204800u64 + (version as u64 - 2) * 8192;
        format!(r#",
        {{
            "id": 1003,
            "name": "Graph Traversal Slides",
            "modname": "resource",
            "description": false,
            "url": "http://localhost:8888/pluginfile.php/101/slides.pdf",
            "contents": [
                {{
                    "filename": "graph-traversal-slides.pdf",
                    "fileurl": "http://localhost:8888/pluginfile.php/101/slides.pdf",
                    "filesize": {slide_size},
                    "mimetype": "application/pdf"
                }}
            ],
            "mainpage": false
        }}"#)
    } else {
        String::new()
    };

    // File size increments with each version so every bump also shows a file_updated diff
    let notes_filesize = 81920u64 + (version as u64).saturating_sub(1) * 4096;

    format!(r#"[
        {{
            "id": 1,
            "name": "General",
            "summary": "<p>Welcome to CS101</p>",
            "modules": [
                {{
                    "id": 1001,
                    "name": "Course Introduction",
                    "modname": "page",
                    "description": false,
                    "url": "http://localhost:8888/mod/page/view.php?id=1001",
                    "contents": [],
                    "mainpage": "<p>{week1_desc}</p>"
                }}
            ]
        }},
        {{
            "id": 2,
            "name": "Week 1 - Fundamentals",
            "summary": "<p>Basic concepts and sorting algorithms</p>",
            "modules": [
                {{
                    "id": 1002,
                    "name": "Week 1 Notes",
                    "modname": "resource",
                    "description": "<p>Lecture notes for Week 1</p>",
                    "url": "http://localhost:8888/pluginfile.php/101/notes.pdf",
                    "contents": [
                        {{
                            "filename": "week1-notes.pdf",
                            "fileurl": "http://localhost:8888/pluginfile.php/101/notes.pdf",
                            "filesize": {notes_filesize},
                            "mimetype": "application/pdf"
                        }}
                    ],
                    "mainpage": false
                }}{week2_module}
            ]
        }}
    ]"#)
}

fn assign_status() -> String {
    r#"{
        "gradingsummary": {"participantcount": 0, "submissiondraftscount": 0, "submissionsenabled": true, "submissionssubmittedcount": 0, "submissionsneedgradingcount": 0, "warnofungroupedusers": ""},
        "lastattempt": null,
        "feedback": null,
        "previousattempts": []
    }"#.into()
}

fn notifications(count: u32) -> String {
    let notifs: String = (0..count).map(|i| format!(
        r#"{{"id":{id},"useridfrom":2,"useridto":1,"subject":"Test notification {num}","fullmessage":"This is test notification {num} from the mock server.","timecreated":{ts},"read":false,"deleted":false,"iconurl":"","component":"moodle","eventtype":"test","contexturl":"","contexturlname":""}}"#,
        id = 1000 + i,
        num = i + 1,
        ts = chrono::Utc::now().timestamp() - (i as i64 * 60),
    )).collect::<Vec<_>>().join(",");

    format!(r#"{{"notifications":[{notifs}],"unreadcount":{count},"warnings":[]}}"#)
}

fn user_profile() -> String {
    r#"[{
        "id": 1,
        "username": "testuser",
        "firstname": "Test",
        "lastname": "Student",
        "fullname": "Test Student",
        "email": "test@example.com",
        "profileimageurl": "",
        "profileimageurlsmall": ""
    }]"#.into()
}

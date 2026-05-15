pub mod types;

use anyhow::{anyhow, bail};
use reqwest::Client;
use types::*;

pub const DEFAULT_BASE: &str = "https://sci.cmb.ac.lk/lms";

static RUNTIME_BASE: std::sync::OnceLock<std::sync::RwLock<String>> = std::sync::OnceLock::new();

fn effective_base() -> String {
    RUNTIME_BASE.get_or_init(|| {
        let v = std::env::var("CAMPUS_LMS_BASE_URL")
            .unwrap_or_else(|_| DEFAULT_BASE.into());
        std::sync::RwLock::new(v)
    }).read().unwrap().clone()
}

pub fn set_moodle_base(url: String) {
    let lock = RUNTIME_BASE.get_or_init(|| std::sync::RwLock::new(url.clone()));
    *lock.write().unwrap() = url;
}

pub fn get_moodle_base() -> String {
    effective_base()
}

#[derive(Clone)]
pub struct MoodleClient {
    http: Client,
    pub token: String,
}

impl MoodleClient {
    pub async fn login(username: &str, password: &str) -> anyhow::Result<(String, String, SiteInfo)> {
        let base = effective_base();
        let http = Client::new();
        let resp: TokenResponse = http
            .post(format!("{base}/login/token.php"))
            .form(&[
                ("username", username),
                ("password", password),
                ("service", "moodle_mobile_app"),
            ])
            .send().await?.json().await?;

        if let Some(err) = resp.error { bail!("Login failed: {err}"); }
        let token = resp.token.ok_or_else(|| anyhow!("No token"))?;
        let private_token = resp.privatetoken.unwrap_or_default();
        let client = Self::new(token.clone());
        let info = client.site_info().await?;
        Ok((token, private_token, info))
    }

    pub fn new(token: String) -> Self {
        Self { http: Client::new(), token }
    }

    fn base_params(&self, wsfunction: &str) -> Vec<(String, String)> {
        vec![
            ("wstoken".into(), self.token.clone()),
            ("wsfunction".into(), wsfunction.into()),
            ("moodlewsrestformat".into(), "json".into()),
        ]
    }

    pub async fn site_info(&self) -> anyhow::Result<SiteInfo> {
        let info: SiteInfo = self.http
            .get(format!("{}/webservice/rest/server.php", effective_base()))
            .query(&self.base_params("core_webservice_get_site_info"))
            .send().await?.json().await?;
        if let Some(code) = &info.errorcode { bail!("token_invalid:{code}"); }
        Ok(info)
    }

    pub async fn get_autologin_url(&self, privatetoken: &str) -> anyhow::Result<AutoLoginResponse> {
        let mut p = self.base_params("tool_mobile_get_autologin_key");
        p.push(("privatetoken".into(), privatetoken.into()));
        Ok(self.http.get(format!("{}/webservice/rest/server.php", effective_base()))
            .query(&p).send().await?.json().await?)
    }

    pub async fn enrolled_courses(&self, userid: u64) -> anyhow::Result<Vec<Course>> {
        let mut p = self.base_params("core_enrol_get_users_courses");
        p.push(("userid".into(), userid.to_string()));
        Ok(self.http.get(format!("{}/webservice/rest/server.php", effective_base()))
            .query(&p).send().await?.json().await?)
    }

    pub async fn course_contents(&self, course_id: u64) -> anyhow::Result<Vec<CourseSection>> {
        // Try with returncontents=1; fall back silently if the course doesn't support it
        match self.fetch_sections(course_id, true).await {
            Err(e) if is_returncontents_error(&e) => self.fetch_sections(course_id, false).await,
            other => other,
        }
    }

    async fn fetch_sections(&self, course_id: u64, with_contents: bool) -> anyhow::Result<Vec<CourseSection>> {
        let mut p = self.base_params("core_course_get_contents");
        p.push(("courseid".into(), course_id.to_string()));
        if with_contents {
            p.push(("options[0][name]".into(), "returncontents".into()));
            p.push(("options[0][value]".into(), "1".into()));
        }
        let text = self.http.get(format!("{}/webservice/rest/server.php", effective_base()))
            .query(&p).send().await?.text().await?;
        if text.trim_start().starts_with('{') {
            let v: serde_json::Value = serde_json::from_str(&text)?;
            let msg = v.get("message").and_then(|m| m.as_str()).unwrap_or("unknown");
            anyhow::bail!("API:{msg}");
        }
        serde_json::from_str::<Vec<CourseSection>>(&text).map_err(|e| {
            let snippet: String = text.chars().take(500).collect();
            anyhow::anyhow!("decode: {e} | {snippet}")
        })
    }

    pub async fn assignments(&self, course_ids: &[u64]) -> anyhow::Result<AssignmentsResponse> {
        let mut p = self.base_params("mod_assign_get_assignments");
        for (i, id) in course_ids.iter().enumerate() {
            p.push((format!("courseids[{i}]"), id.to_string()));
        }
        Ok(self.http.get(format!("{}/webservice/rest/server.php", effective_base()))
            .query(&p).send().await?.json().await?)
    }

    pub async fn submission_status(&self, assign_id: u64) -> anyhow::Result<SubmissionStatusResponse> {
        let mut p = self.base_params("mod_assign_get_submission_status");
        p.push(("assignid".into(), assign_id.to_string()));
        Ok(self.http.get(format!("{}/webservice/rest/server.php", effective_base()))
            .query(&p).send().await?.json().await?)
    }

    /// Upload a file to the user's draft area. Returns the draft itemid.
    pub async fn upload_file(&self, filename: &str, data: Vec<u8>) -> anyhow::Result<FileUploadResponse> {
        let part = reqwest::multipart::Part::bytes(data)
            .file_name(filename.to_string())
            .mime_str("application/octet-stream")?;
        let form = reqwest::multipart::Form::new()
            .text("token", self.token.clone())
            .text("filearea", "draft")
            .text("itemid", "0")
            .text("filepath", "/")
            .text("filename", filename.to_string())
            .part("file_1", part);
        let resp: serde_json::Value = self.http
            .post(format!("{}/webservice/upload.php", effective_base()))
            .multipart(form)
            .send().await?.json().await?;
        // API returns an array with one item
        let item = resp.get(0).or_else(|| Some(&resp))
            .ok_or_else(|| anyhow::anyhow!("empty upload response"))?;
        Ok(FileUploadResponse {
            itemid: item.get("itemid").and_then(|v| v.as_u64()).unwrap_or(0),
            filename: item.get("filename").and_then(|v| v.as_str()).unwrap_or(filename).to_string(),
        })
    }

    /// Save a file submission for an assignment using a draft itemid.
    pub async fn save_submission(&self, assign_id: u64, itemid: u64) -> anyhow::Result<()> {
        let mut p = self.base_params("mod_assign_save_submission");
        p.push(("assignmentid".into(), assign_id.to_string()));
        p.push(("plugindata[files_filemanager]".into(), itemid.to_string()));
        let _: serde_json::Value = self.http
            .post(format!("{}/webservice/rest/server.php", effective_base()))
            .form(&p).send().await?.json().await?;
        Ok(())
    }

    /// Submit the saved draft for grading.
    pub async fn submit_for_grading(&self, assign_id: u64) -> anyhow::Result<()> {
        let mut p = self.base_params("mod_assign_submit_for_grading");
        p.push(("assignmentid".into(), assign_id.to_string()));
        p.push(("acceptsubmissionstatement".into(), "1".into()));
        let _: serde_json::Value = self.http
            .post(format!("{}/webservice/rest/server.php", effective_base()))
            .form(&p).send().await?.json().await?;
        Ok(())
    }

    pub async fn forums_by_courses(&self, course_ids: &[u64]) -> anyhow::Result<Vec<Forum>> {
        let mut p = self.base_params("mod_forum_get_forums_by_courses");
        for (i, id) in course_ids.iter().enumerate() {
            p.push((format!("courseids[{i}]"), id.to_string()));
        }
        Ok(self.http.get(format!("{}/webservice/rest/server.php", effective_base()))
            .query(&p).send().await?.json().await?)
    }

    pub async fn forum_discussions(&self, forum_id: u64) -> anyhow::Result<ForumDiscussionsResponse> {
        let mut p = self.base_params("mod_forum_get_forum_discussions_paginated");
        p.push(("forumid".into(), forum_id.to_string()));
        p.push(("perpage".into(), "20".into()));
        Ok(self.http.get(format!("{}/webservice/rest/server.php", effective_base()))
            .query(&p).send().await?.json().await?)
    }

    pub async fn calendar_events(&self, from: i64, to: i64) -> anyhow::Result<CalendarEventList> {
        let mut p = self.base_params("core_calendar_get_action_events_by_timesort");
        p.push(("timesortfrom".into(), from.to_string()));
        p.push(("timesortto".into(), to.to_string()));
        p.push(("limitnum".into(), "200".into()));
        Ok(self.http.get(format!("{}/webservice/rest/server.php", effective_base()))
            .query(&p).send().await?.json().await?)
    }

    pub async fn notifications(&self, userid: u64, offset: u32) -> anyhow::Result<NotificationList> {
        let mut p = self.base_params("message_popup_get_popup_notifications");
        p.push(("useridto".into(), userid.to_string()));
        p.push(("offset".into(), offset.to_string()));
        p.push(("limit".into(), "50".into()));
        Ok(self.http.get(format!("{}/webservice/rest/server.php", effective_base()))
            .query(&p).send().await?.json().await?)
    }

    pub async fn user_profile(&self, userid: u64) -> anyhow::Result<UserProfile> {
        let mut p = self.base_params("core_user_get_users_by_field");
        p.push(("field".into(), "id".into()));
        p.push(("values[0]".into(), userid.to_string()));
        let users: Vec<UserProfile> = self.http
            .get(format!("{}/webservice/rest/server.php", effective_base()))
            .query(&p).send().await?.json().await?;
        users.into_iter().next().ok_or_else(|| anyhow::anyhow!("Profile not found"))
    }

    pub async fn grades_overview(&self, userid: u64) -> anyhow::Result<GradeOverviewResponse> {
        let mut p = self.base_params("gradereport_overview_get_course_grades");
        p.push(("userid".into(), userid.to_string()));
        Ok(self.http.get(format!("{}/webservice/rest/server.php", effective_base()))
            .query(&p).send().await?.json().await?)
    }

    pub async fn grades(&self, userid: u64, course_id: u64) -> anyhow::Result<GradeItemsResponse> {
        let mut p = self.base_params("gradereport_user_get_grade_items");
        p.push(("userid".into(), userid.to_string()));
        p.push(("courseid".into(), course_id.to_string()));
        Ok(self.http.get(format!("{}/webservice/rest/server.php", effective_base()))
            .query(&p).send().await?.json().await?)
    }
}

pub fn is_token_error(e: &anyhow::Error) -> bool {
    let s = e.to_string();
    s.contains("token_invalid") || s.contains("invalidtoken") || s.contains("Invalid token")
}

fn is_returncontents_error(e: &anyhow::Error) -> bool {
    let s = e.to_string();
    s.contains("returncontents") || s.contains("param") && s.contains("invalid")
}

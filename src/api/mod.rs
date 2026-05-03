pub mod types;

use anyhow::{anyhow, bail};
use reqwest::Client;
use types::*;

const BASE: &str = "https://sci.cmb.ac.lk/lms";

#[derive(Clone)]
pub struct MoodleClient {
    http: Client,
    pub token: String,
}

impl MoodleClient {
    pub async fn login(username: &str, password: &str) -> anyhow::Result<(String, String, SiteInfo)> {
        let http = Client::new();
        let resp: TokenResponse = http
            .post(format!("{BASE}/login/token.php"))
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
            .get(format!("{BASE}/webservice/rest/server.php"))
            .query(&self.base_params("core_webservice_get_site_info"))
            .send().await?.json().await?;
        if let Some(code) = &info.errorcode { bail!("token_invalid:{code}"); }
        Ok(info)
    }

    pub async fn get_autologin_url(&self, privatetoken: &str) -> anyhow::Result<AutoLoginResponse> {
        let mut p = self.base_params("tool_mobile_get_autologin_key");
        p.push(("privatetoken".into(), privatetoken.into()));
        Ok(self.http.get(format!("{BASE}/webservice/rest/server.php"))
            .query(&p).send().await?.json().await?)
    }

    pub async fn enrolled_courses(&self, userid: u64) -> anyhow::Result<Vec<Course>> {
        let mut p = self.base_params("core_enrol_get_users_courses");
        p.push(("userid".into(), userid.to_string()));
        Ok(self.http.get(format!("{BASE}/webservice/rest/server.php"))
            .query(&p).send().await?.json().await?)
    }

    pub async fn course_contents(&self, course_id: u64) -> anyhow::Result<Vec<CourseSection>> {
        let mut p = self.base_params("core_course_get_contents");
        p.push(("courseid".into(), course_id.to_string()));
        // Return full page HTML content so "page" modules can be shown inline
        p.push(("options[0][name]".into(), "returncontents".into()));
        p.push(("options[0][value]".into(), "1".into()));
        Ok(self.http.get(format!("{BASE}/webservice/rest/server.php"))
            .query(&p).send().await?.json().await?)
    }

    pub async fn assignments(&self, course_ids: &[u64]) -> anyhow::Result<AssignmentsResponse> {
        let mut p = self.base_params("mod_assign_get_assignments");
        for (i, id) in course_ids.iter().enumerate() {
            p.push((format!("courseids[{i}]"), id.to_string()));
        }
        Ok(self.http.get(format!("{BASE}/webservice/rest/server.php"))
            .query(&p).send().await?.json().await?)
    }

    pub async fn submission_status(&self, assign_id: u64) -> anyhow::Result<SubmissionStatusResponse> {
        let mut p = self.base_params("mod_assign_get_submission_status");
        p.push(("assignid".into(), assign_id.to_string()));
        Ok(self.http.get(format!("{BASE}/webservice/rest/server.php"))
            .query(&p).send().await?.json().await?)
    }

    pub async fn forums_by_courses(&self, course_ids: &[u64]) -> anyhow::Result<Vec<Forum>> {
        let mut p = self.base_params("mod_forum_get_forums_by_courses");
        for (i, id) in course_ids.iter().enumerate() {
            p.push((format!("courseids[{i}]"), id.to_string()));
        }
        Ok(self.http.get(format!("{BASE}/webservice/rest/server.php"))
            .query(&p).send().await?.json().await?)
    }

    pub async fn forum_discussions(&self, forum_id: u64) -> anyhow::Result<ForumDiscussionsResponse> {
        let mut p = self.base_params("mod_forum_get_forum_discussions_paginated");
        p.push(("forumid".into(), forum_id.to_string()));
        p.push(("perpage".into(), "20".into()));
        Ok(self.http.get(format!("{BASE}/webservice/rest/server.php"))
            .query(&p).send().await?.json().await?)
    }

    pub async fn calendar_events(&self, from: i64, to: i64) -> anyhow::Result<CalendarEventList> {
        let mut p = self.base_params("core_calendar_get_action_events_by_timesort");
        p.push(("timesortfrom".into(), from.to_string()));
        p.push(("timesortto".into(), to.to_string()));
        p.push(("limitnum".into(), "200".into()));
        Ok(self.http.get(format!("{BASE}/webservice/rest/server.php"))
            .query(&p).send().await?.json().await?)
    }

    pub async fn notifications(&self, userid: u64, offset: u32) -> anyhow::Result<NotificationList> {
        let mut p = self.base_params("message_popup_get_popup_notifications");
        p.push(("useridto".into(), userid.to_string()));
        p.push(("offset".into(), offset.to_string()));
        p.push(("limit".into(), "50".into()));
        Ok(self.http.get(format!("{BASE}/webservice/rest/server.php"))
            .query(&p).send().await?.json().await?)
    }

    pub async fn user_profile(&self, userid: u64) -> anyhow::Result<UserProfile> {
        let mut p = self.base_params("core_user_get_users_by_field");
        p.push(("field".into(), "id".into()));
        p.push(("values[0]".into(), userid.to_string()));
        let users: Vec<UserProfile> = self.http
            .get(format!("{BASE}/webservice/rest/server.php"))
            .query(&p).send().await?.json().await?;
        users.into_iter().next().ok_or_else(|| anyhow::anyhow!("Profile not found"))
    }

    pub async fn grades_overview(&self, userid: u64) -> anyhow::Result<GradeOverviewResponse> {
        let mut p = self.base_params("gradereport_overview_get_course_grades");
        p.push(("userid".into(), userid.to_string()));
        Ok(self.http.get(format!("{BASE}/webservice/rest/server.php"))
            .query(&p).send().await?.json().await?)
    }

    pub async fn grades(&self, userid: u64, course_id: u64) -> anyhow::Result<GradeItemsResponse> {
        let mut p = self.base_params("gradereport_user_get_grade_items");
        p.push(("userid".into(), userid.to_string()));
        p.push(("courseid".into(), course_id.to_string()));
        Ok(self.http.get(format!("{BASE}/webservice/rest/server.php"))
            .query(&p).send().await?.json().await?)
    }
}

pub fn is_token_error(e: &anyhow::Error) -> bool {
    let s = e.to_string();
    s.contains("token_invalid") || s.contains("invalidtoken") || s.contains("Invalid token")
}

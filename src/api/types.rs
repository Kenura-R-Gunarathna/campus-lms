#![allow(dead_code)]
use serde::Deserialize;

fn de_false_as_none<'de, D>(d: D) -> Result<Option<String>, D::Error>
where D: serde::Deserializer<'de> {
    let v: serde_json::Value = Deserialize::deserialize(d)?;
    Ok(match v {
        serde_json::Value::String(s) if !s.is_empty() => Some(s),
        _ => None,
    })
}

fn de_false_as_zero<'de, D>(d: D) -> Result<i64, D::Error>
where D: serde::Deserializer<'de> {
    let v: serde_json::Value = Deserialize::deserialize(d)?;
    Ok(match v {
        serde_json::Value::Number(n) => n.as_i64().unwrap_or(0),
        _ => 0,
    })
}

// ── Auth ────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub token: Option<String>,
    pub privatetoken: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SiteInfo {
    pub userid: u64,
    pub fullname: String,
    pub sitename: String,
    pub errorcode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AutoLoginResponse {
    pub key: String,
    pub autologinurl: String,
}

// ── Courses ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct Course {
    pub id: u64,
    pub fullname: String,
    pub shortname: String,
    #[serde(default)]
    pub categoryid: u64,
    #[serde(default)]
    pub categoryname: String,
    #[serde(default)]
    pub summary: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CourseSection {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub modules: Vec<CourseModule>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CourseModule {
    pub id: u64,
    pub name: String,
    pub modname: String,
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub description: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub contents: Vec<ModuleContent>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ModuleContent {
    pub filename: String,
    pub fileurl: String,
    pub filesize: u64,
    pub mimetype: Option<String>,
}

// ── Assignments ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct AssignmentsResponse {
    pub courses: Vec<AssignmentCourse>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AssignmentCourse {
    pub id: u64,
    pub shortname: String,
    pub fullname: String,
    #[serde(default)]
    pub assignments: Vec<Assignment>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Assignment {
    pub id: u64,
    pub cmid: u64,
    pub name: String,
    #[serde(default)]
    pub duedate: i64,
    #[serde(default)]
    pub cutoffdate: i64,
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub intro: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SubmissionStatusResponse {
    pub lastattempt: Option<LastAttempt>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LastAttempt {
    pub submission: Option<Submission>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Submission {
    pub id: u64,
    pub status: String, // "new", "draft", "submitted"
    pub timemodified: i64,
}

// ── Forums / Announcements ──────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct Forum {
    pub id: u64,
    pub course: u64,
    #[serde(rename = "type")]
    pub forum_type: String, // "news", "general", etc.
    pub name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ForumDiscussionsResponse {
    pub discussions: Vec<ForumDiscussion>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ForumDiscussion {
    pub id: u64,
    pub name: String,
    pub message: String,
    pub userfullname: String,
    pub usermodifiedfullname: String,
    pub timecreated: i64,
    pub timemodified: i64,
    pub numreplies: u32,
    pub pinned: bool,
    pub subject: String,
    pub messageformat: u32,
}

// ── Calendar ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct CalendarEventList {
    pub events: Vec<CalendarEvent>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CalendarEvent {
    pub id: u64,
    pub name: String,
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub description: Option<String>,
    #[serde(default)]
    pub timestart: i64,
    #[serde(default)]
    pub timesort: i64,
    #[serde(default)]
    pub courseid: u64,
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub coursename: Option<String>,
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub modulename: Option<String>,
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub eventtype: Option<String>,
}

// ── Notifications ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct NotificationList {
    pub notifications: Vec<MoodleNotification>,
    #[serde(default)]
    pub unreadcount: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MoodleNotification {
    pub id: u64,
    pub subject: String,
    #[serde(default)]
    pub fullmessage: String,
    #[serde(default)]
    pub timecreated: i64,
    #[serde(rename = "read")]
    pub is_read: bool,
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub contexturl: Option<String>,
}

// ── User Profile ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct UserProfile {
    pub id: u64,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub firstname: String,
    #[serde(default)]
    pub lastname: String,
    #[serde(default)]
    pub fullname: String,
    #[serde(default)]
    pub email: String,
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub idnumber: Option<String>,
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub description: Option<String>,
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub profileimageurl: Option<String>,
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub city: Option<String>,
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub country: Option<String>,
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub phone1: Option<String>,
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub phone2: Option<String>,
}

// ── Grade Overview ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct GradeOverviewResponse {
    pub grades: Vec<CourseGradeOverview>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CourseGradeOverview {
    pub courseid: u64,
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub grade: Option<String>,
}

// ── Grades ────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct GradeItemsResponse {
    pub usergrades: Vec<UserGrades>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct UserGrades {
    pub courseid: u64,
    pub coursename: String,
    #[serde(default)]
    pub gradeitems: Vec<GradeItem>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GradeItem {
    pub id: u64,
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub itemname: Option<String>,
    #[serde(default)]
    pub gradeformatted: String,
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub percentageformatted: Option<String>,
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub feedback: Option<String>,
    #[serde(default, deserialize_with = "de_false_as_zero")]
    pub gradedategraded: i64,
}

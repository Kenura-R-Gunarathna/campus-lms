#![allow(dead_code)]
use serde::{Deserialize, Serialize};

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

fn de_false_as_zero_u64<'de, D>(d: D) -> Result<u64, D::Error>
where D: serde::Deserializer<'de> {
    let v: serde_json::Value = Deserialize::deserialize(d)?;
    Ok(match v {
        serde_json::Value::Number(n) => n.as_u64().unwrap_or(0),
        _ => 0,
    })
}

fn de_false_as_empty<'de, D>(d: D) -> Result<String, D::Error>
where D: serde::Deserializer<'de> {
    let v: serde_json::Value = Deserialize::deserialize(d)?;
    Ok(match v {
        serde_json::Value::String(s) => s,
        _ => String::new(),
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

#[derive(Debug, Deserialize, Serialize, Clone)]
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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CourseSection {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub modules: Vec<CourseModule>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CourseModule {
    pub id: u64,
    pub name: String,
    pub modname: String,
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub description: Option<String>,
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub url: Option<String>,
    #[serde(default)]
    pub contents: Vec<ModuleContent>,
    /// Full HTML content (only present when returncontents=1, e.g. "page" modules)
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub mainpage: Option<String>,
    #[serde(default)]
    pub dates: Vec<ModuleDate>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModuleDate {
    pub label: String,
    pub timestamp: i64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModuleContent {
    #[serde(deserialize_with = "de_false_as_empty", default)]
    pub filename: String,
    #[serde(deserialize_with = "de_false_as_empty", default)]
    pub fileurl: String,
    #[serde(deserialize_with = "de_false_as_zero_u64", default)]
    pub filesize: u64,
    #[serde(deserialize_with = "de_false_as_zero_u64", default)]
    pub timemodified: u64,
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub mimetype: Option<String>,
}

// ── Assignments ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct AssignmentsResponse {
    pub courses: Vec<AssignmentCourse>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AssignmentCourse {
    pub id: u64,
    pub shortname: String,
    pub fullname: String,
    #[serde(default)]
    pub assignments: Vec<Assignment>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
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
    #[serde(default)]
    pub introattachments: Vec<IntroAttachment>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IntroAttachment {
    #[serde(deserialize_with = "de_false_as_empty", default)]
    pub filename: String,
    #[serde(deserialize_with = "de_false_as_empty", default)]
    pub fileurl: String,
    #[serde(deserialize_with = "de_false_as_zero_u64", default)]
    pub filesize: u64,
    #[serde(deserialize_with = "de_false_as_none", default)]
    pub mimetype: Option<String>,
}

impl Assignment {
    pub fn url(&self) -> Option<String> {
        if self.cmid == 0 { None } else {
            Some(format!("https://sci.cmb.ac.lk/lms/mod/assign/view.php?id={}", self.cmid))
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct SubmissionStatusResponse {
    pub lastattempt: Option<LastAttempt>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LastAttempt {
    pub submission: Option<Submission>,
    #[serde(default)]
    pub gradingstatus: String, // "notgraded", "graded"
    #[serde(default)]
    pub submissionsenabled: bool,
    #[serde(default)]
    pub canedit: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Submission {
    pub id: u64,
    pub status: String, // "new", "draft", "submitted"
    pub timemodified: i64,
    #[serde(default)]
    pub plugins: Vec<SubmissionPlugin>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SubmissionPlugin {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub fileareas: Vec<SubmissionFileArea>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SubmissionFileArea {
    #[serde(default)]
    pub area: String,
    #[serde(default)]
    pub files: Vec<SubmissionFile>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SubmissionFile {
    #[serde(deserialize_with = "de_false_as_empty", default)]
    pub filename: String,
    #[serde(deserialize_with = "de_false_as_empty", default)]
    pub fileurl: String,
    #[serde(deserialize_with = "de_false_as_zero_u64", default)]
    pub filesize: u64,
    #[serde(default)]
    pub timemodified: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FileUploadResponse {
    pub itemid: u64,
    #[serde(default)]
    pub filename: String,
}

// ── Forums / Announcements ──────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Forum {
    pub id: u64,
    pub course: u64,
    #[serde(rename = "type")]
    pub forum_type: String, // "news", "general", etc.
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ForumDiscussionsResponse {
    pub discussions: Vec<ForumDiscussion>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CalendarEventList {
    pub events: Vec<CalendarEvent>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NotificationList {
    pub notifications: Vec<MoodleNotification>,
    #[serde(default)]
    pub unreadcount: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UserGrades {
    pub courseid: u64,
    pub coursename: String,
    #[serde(default)]
    pub gradeitems: Vec<GradeItem>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
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

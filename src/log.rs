/// Shared log entry types used by app.rs and the activity log screen.

#[derive(Clone, PartialEq)]
pub enum LogLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone)]
pub struct LogEntry {
    pub timestamp: i64,
    pub level: LogLevel,
    pub category: &'static str, // "diff" | "notification" | "upload" | "download" | "auth" | "system"
    pub message: String,
}

impl LogEntry {
    pub fn new(level: LogLevel, category: &'static str, message: impl Into<String>) -> Self {
        Self {
            timestamp: chrono::Utc::now().timestamp(),
            level,
            category,
            message: message.into(),
        }
    }

    pub fn info(category: &'static str, msg: impl Into<String>) -> Self {
        Self::new(LogLevel::Info, category, msg)
    }
    pub fn success(category: &'static str, msg: impl Into<String>) -> Self {
        Self::new(LogLevel::Success, category, msg)
    }
    pub fn warn(category: &'static str, msg: impl Into<String>) -> Self {
        Self::new(LogLevel::Warning, category, msg)
    }
    pub fn error(category: &'static str, msg: impl Into<String>) -> Self {
        Self::new(LogLevel::Error, category, msg)
    }
}

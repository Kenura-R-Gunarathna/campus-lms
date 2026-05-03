use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Instant;
use crate::api::{is_token_error, types::*, MoodleClient};
use crate::background;
use crate::models::{decode_html, infer_student_year, parse_dept, year_label};
use crate::screens::{
    announcements::{Announcement, AnnouncementsScreen},
    assignment_detail::{AssignmentDetailScreen, AssignmentDetailEvent, DetailSource, UploadState, extract_intro_images},
    assignments::AssignmentsScreen,
    calendar::CalendarScreen,
    courses::{CoursesEvent, CoursesScreen},
    course_content::{CourseContentScreen, CourseContentEvent, DownloadState},
    grades::GradesScreen,
    login::LoginScreen,
    notifications::NotificationsScreen,
    profile::ProfileScreen,
};
use crate::storage::{Storage, ActivityEntry};

const KEYRING_SERVICE: &str = "campus-lms";

#[derive(PartialEq, Clone, Copy)]
enum Tab { Courses, Announcements, Assignments, Calendar, Notifications, Grades, Profile }

impl Tab {
    fn label(self) -> &'static str {
        match self {
            Tab::Courses       => "Courses",
            Tab::Announcements => "Announcements",
            Tab::Assignments   => "Assignments",
            Tab::Calendar      => "Calendar",
            Tab::Notifications => "Notifications",
            Tab::Grades        => "Grades",
            Tab::Profile       => "Profile",
        }
    }
}
const TABS: &[Tab] = &[
    Tab::Courses, Tab::Announcements, Tab::Assignments, Tab::Calendar,
    Tab::Notifications, Tab::Grades, Tab::Profile,
];

enum Screen { Login, Main }

enum AppMsg {
    LoginOk { token: String, private_token: String, info: SiteInfo },
    LoginErr(String),
    CoursesLoaded(Vec<Course>),
    CourseContentLoaded { course_id: u64, sections: Vec<CourseSection> },
    AnnouncementsLoaded(Vec<Announcement>),
    AssignmentsLoaded(AssignmentsResponse),
    AssignmentStatusLoaded { assign_id: u64, status: SubmissionStatusResponse },
    CalendarLoaded(CalendarEventList),
    NotificationsLoaded(NotificationList),
    GradeOverviewLoaded(GradeOverviewResponse),
    GradesDetailLoaded(Vec<UserGrades>),
    ProfileLoaded(UserProfile),
    TokenExpired,
    NewNotifications(u64),
    OpenUrl(String),
    FileDownloaded { module_id: u64, path: PathBuf },
    FileDownloadFailed { module_id: u64, error: String },
    FilePicked { assign_id: u64, filename: String, data: Vec<u8> },
    AssignmentUploadDone { assign_id: u64 },
    AssignmentUploadFailed { assign_id: u64, error: String },
}


pub struct App {
    screen: Screen,
    active_tab: Tab,
    login: LoginScreen,
    courses: CoursesScreen,
    course_content: CourseContentScreen,
    announcements: AnnouncementsScreen,
    assignments: AssignmentsScreen,
    assignment_detail: AssignmentDetailScreen,
    calendar: CalendarScreen,
    notifications: NotificationsScreen,
    grades: GradesScreen,
    profile: ProfileScreen,
    storage: Storage,
    token: String,
    private_token: String,
    userid: u64,
    fullname: String,
    student_year: Option<u8>,
    // Time tracking
    focused_course: Option<u64>,
    focus_since: Option<Instant>,
    // Data loaded flags
    announcements_loaded: bool,
    assignments_loaded: bool,
    calendar_loaded: bool,
    notifications_loaded: bool,
    grades_loaded: bool,
    profile_loaded: bool,
    // Settings panel
    show_settings: bool,
    bg_enabled: bool,
    tx: Sender<AppMsg>,
    rx: Receiver<AppMsg>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (tx, rx) = channel();
        let storage = Storage::open().expect("storage init failed");
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);

        let bg_enabled = background::autostart_enabled();

        let mut app = Self {
            screen: Screen::Login,
            active_tab: Tab::Courses,
            login: LoginScreen::default(),
            courses: CoursesScreen::default(),
            course_content: CourseContentScreen::default(),
            announcements: AnnouncementsScreen::default(),
            assignments: AssignmentsScreen::default(),
            assignment_detail: AssignmentDetailScreen::default(),
            calendar: CalendarScreen::default(),
            notifications: NotificationsScreen::default(),
            grades: GradesScreen::default(),
            profile: ProfileScreen::default(),
            storage,
            token: String::new(),
            private_token: String::new(),
            userid: 0,
            fullname: String::new(),
            student_year: None,
            focused_course: None,
            focus_since: None,
            announcements_loaded: false,
            assignments_loaded: false,
            calendar_loaded: false,
            notifications_loaded: false,
            grades_loaded: false,
            profile_loaded: false,
            show_settings: false,
            bg_enabled,
            tx,
            rx,
        };

        if let (Ok(Some(token)), Ok(Some(uid))) = (
            app.storage.get("token"),
            app.storage.get("userid"),
        ) {
            if let Ok(userid) = uid.parse::<u64>() {
                app.token = token.clone();
                let private_token = app.storage.get("private_token").ok().flatten().unwrap_or_default();
                app.private_token = private_token.clone();
                app.userid = userid;
                app.fullname = app.storage.get("fullname").ok().flatten().unwrap_or_default();
                app.screen = Screen::Main;

                // Load unseen change counts + recent activity
                if let Ok(counts) = app.storage.unseen_change_counts() {
                    app.courses.change_counts = counts;
                }
                if let Ok(ra) = app.storage.recent_activity(5) {
                    app.courses.recent_activity = ra;
                }

                // Pre-populate from cache (stale-while-revalidate)
                if let Ok(Some(json)) = app.storage.load_cache("courses") {
                    if let Ok(v) = serde_json::from_str::<Vec<Course>>(&json) {
                        app.courses.courses = v;
                    }
                }
                if let Ok(Some(json)) = app.storage.load_cache("announcements") {
                    if let Ok(v) = serde_json::from_str::<Vec<Announcement>>(&json) {
                        app.announcements.announcements = v;
                    }
                }
                if let Ok(Some(json)) = app.storage.load_cache("assignments") {
                    if let Ok(v) = serde_json::from_str::<Vec<AssignmentCourse>>(&json) {
                        app.assignments.courses = v;
                    }
                }
                if let Ok(Some(json)) = app.storage.load_cache("calendar") {
                    if let Ok(v) = serde_json::from_str::<Vec<CalendarEvent>>(&json) {
                        app.calendar.events = v;
                    }
                }
                if let Ok(Some(json)) = app.storage.load_cache("notifications") {
                    if let Ok(v) = serde_json::from_str::<Vec<MoodleNotification>>(&json) {
                        app.notifications.notifications = v;
                    }
                }
                if let Ok(Some(json)) = app.storage.load_cache("grades") {
                    if let Ok(v) = serde_json::from_str::<Vec<UserGrades>>(&json) {
                        app.grades.grades = v;
                    }
                }

                let tx = app.tx.clone();
                let username = app.storage.get("username").ok().flatten().unwrap_or_default();
                let needs_private_token = private_token.is_empty();
                tokio::spawn(async move {
                    let client = MoodleClient::new(token.clone());
                    match client.enrolled_courses(userid).await {
                        Ok(courses) => { 
                            let _ = tx.send(AppMsg::CoursesLoaded(courses)); 
                            if needs_private_token {
                                if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, &username) {
                                    if let Ok(password) = entry.get_password() {
                                        if let Ok((t, pt, info)) = MoodleClient::login(&username, &password).await {
                                            let _ = tx.send(AppMsg::LoginOk { token: t, private_token: pt, info });
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) if is_token_error(&e) => {
                            if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, &username) {
                                if let Ok(password) = entry.get_password() {
                                    match MoodleClient::login(&username, &password).await {
                                        Ok((t, pt, info)) => { let _ = tx.send(AppMsg::LoginOk { token: t, private_token: pt, info }); }
                                        Err(_) => { let _ = tx.send(AppMsg::TokenExpired); }
                                    }
                                    return;
                                }
                            }
                            let _ = tx.send(AppMsg::TokenExpired);
                        }
                        Err(e) => eprintln!("courses fetch: {e}"),
                    }
                });
            }
        }

        app
    }

    fn apply_year_defaults(&mut self) {
        if let Some(y) = self.student_year {
            if self.courses.year_filter.is_none()     { self.courses.year_filter     = Some(y); }
            if self.announcements.year_filter.is_none() { self.announcements.year_filter = Some(y); }
            self.announcements.student_year = Some(y);
            if self.assignments.year_filter.is_none() { self.assignments.year_filter = Some(y); }
            self.assignments.student_year = Some(y);
            if self.grades.year_filter.is_none()      { self.grades.year_filter      = Some(y); }
        }
    }

    fn fetch_announcements(&self) {
        let courses: Vec<(u64, String)> = self.courses.courses.iter()
            .map(|c| (c.id, c.shortname.clone()))
            .collect();
        if courses.is_empty() { return; }
        let token = self.token.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let client = MoodleClient::new(token);
            match client.forums_by_courses(&courses.iter().map(|(id, _)| *id).collect::<Vec<_>>()).await {
                Ok(forums) => {
                    let mut all_ann = vec![];
                    for forum in forums {
                        if forum.forum_type == "news" || forum.name.to_lowercase().contains("announcement") {
                            if let Ok(resp) = client.forum_discussions(forum.id).await {
                                let course_name = courses.iter().find(|(id, _)| *id == forum.course)
                                    .map(|(_, n)| n.clone()).unwrap_or_default();
                                for disc in resp.discussions {
                                    all_ann.push(Announcement {
                                        discussion: disc,
                                        course_id: forum.course,
                                        course_name: course_name.clone(),
                                    });
                                }
                            }
                        }
                    }
                    let _ = tx.send(AppMsg::AnnouncementsLoaded(all_ann));
                }
                Err(e) => eprintln!("forums fetch: {e}"),
            }
        });
    }

    fn fetch_assignments(&self) {
        let ids: Vec<u64> = self.courses.courses.iter().map(|c| c.id).collect();
        if ids.is_empty() { return; }
        let token = self.token.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let client = MoodleClient::new(token);
            match client.assignments(&ids).await {
                Ok(r) => { let _ = tx.send(AppMsg::AssignmentsLoaded(r)); }
                Err(e) => eprintln!("assignments: {e}"),
            }
        });
    }

    fn fetch_assignment_status(&self, assign_id: u64) {
        let token = self.token.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let client = MoodleClient::new(token);
            match client.submission_status(assign_id).await {
                Ok(status) => { let _ = tx.send(AppMsg::AssignmentStatusLoaded { assign_id, status }); }
                Err(e) => eprintln!("submission status {assign_id}: {e}"),
            }
        });
    }

    fn pick_file_for_assignment(&self, assign_id: u64) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                rfd::FileDialog::new().pick_file().and_then(|path| {
                    let filename = path.file_name()?.to_string_lossy().into_owned();
                    let data = std::fs::read(&path).ok()?;
                    Some((filename, data))
                })
            }).await;
            if let Ok(Some((filename, data))) = result {
                let _ = tx.send(AppMsg::FilePicked { assign_id, filename, data });
            }
        });
    }

    fn upload_and_submit(&self, assign_id: u64, filename: String, data: Vec<u8>) {
        let token = self.token.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let client = MoodleClient::new(token);
            let upload = client.upload_file(&filename, data).await;
            match upload {
                Err(e) => { let _ = tx.send(AppMsg::AssignmentUploadFailed { assign_id, error: e.to_string() }); }
                Ok(resp) => {
                    if let Err(e) = client.save_submission(assign_id, resp.itemid).await {
                        let _ = tx.send(AppMsg::AssignmentUploadFailed { assign_id, error: e.to_string() });
                        return;
                    }
                    match client.submit_for_grading(assign_id).await {
                        Ok(()) => { let _ = tx.send(AppMsg::AssignmentUploadDone { assign_id }); }
                        Err(e) => { let _ = tx.send(AppMsg::AssignmentUploadFailed { assign_id, error: e.to_string() }); }
                    }
                }
            }
        });
    }

    fn handle_detail_event(&mut self, ev: AssignmentDetailEvent) {
        match ev {
            AssignmentDetailEvent::Back => {
                self.assignment_detail.assignment = None;
            }
            AssignmentDetailEvent::UploadFile => {
                if let Some(assign) = &self.assignment_detail.assignment {
                    let id = assign.id;
                    self.pick_file_for_assignment(id);
                }
            }
            AssignmentDetailEvent::SubmitForGrading => {
                if let Some((fname, data)) = self.assignment_detail.pending_file.take() {
                    if let Some(assign) = &self.assignment_detail.assignment {
                        let id = assign.id;
                        self.assignment_detail.upload_state = UploadState::Uploading;
                        self.upload_and_submit(id, fname, data);
                    }
                }
            }
            AssignmentDetailEvent::OpenFile { url } => {
                let token = self.token.clone();
                let sep = if url.contains('?') { '&' } else { '?' };
                let _ = open::that(format!("{url}{sep}token={token}"));
            }
            AssignmentDetailEvent::OpenInBrowser { url } => {
                let _ = self.tx.send(AppMsg::OpenUrl(url));
            }
        }
    }

    fn open_assignment_detail(&mut self, assign: crate::api::types::Assignment, course_name: String, source: DetailSource) {
        let assign_id = assign.id;
        let images = assign.intro.as_deref()
            .map(|intro| extract_intro_images(&crate::models::decode_html(intro), assign.id))
            .unwrap_or_default();
        self.assignment_detail.intro_base64_images = images;
        self.assignment_detail.assignment = Some(assign);
        self.assignment_detail.course_name = course_name;
        self.assignment_detail.source = source;
        self.assignment_detail.token = self.token.clone();
        self.assignment_detail.status = self.assignments.submission_statuses.get(&assign_id).cloned();
        self.assignment_detail.loading_status = self.assignment_detail.status.is_none();
        self.assignment_detail.upload_state = UploadState::Idle;
        self.assignment_detail.pending_file = None;
        if self.assignment_detail.status.is_none() && !self.assignments.loading_statuses.contains(&assign_id) {
            self.assignments.loading_statuses.insert(assign_id);
            self.fetch_assignment_status(assign_id);
        }
        if !self.assignments_loaded {
            self.assignments_loaded = true;
            self.fetch_assignments();
        }
    }

    fn fetch_calendar(&self) {
        let token = self.token.clone();
        let tx = self.tx.clone();
        let from = chrono::Utc::now().timestamp() - 90 * 86400;
        let to   = chrono::Utc::now().timestamp() + 180 * 86400;
        tokio::spawn(async move {
            let client = MoodleClient::new(token);
            match client.calendar_events(from, to).await {
                Ok(r) => { let _ = tx.send(AppMsg::CalendarLoaded(r)); }
                Err(e) => eprintln!("calendar: {e}"),
            }
        });
    }

    fn fetch_notifications(&self) {
        let token = self.token.clone();
        let userid = self.userid;
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let client = MoodleClient::new(token);
            match client.notifications(userid, 0).await {
                Ok(r) => { let _ = tx.send(AppMsg::NotificationsLoaded(r)); }
                Err(e) => eprintln!("notifications: {e}"),
            }
        });
    }

    fn fetch_grades_overview(&self) {
        let token = self.token.clone();
        let userid = self.userid;
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let client = MoodleClient::new(token);
            match client.grades_overview(userid).await {
                Ok(r) => { let _ = tx.send(AppMsg::GradeOverviewLoaded(r)); }
                Err(e) => eprintln!("grades_overview: {e}"),
            }
        });
    }

    fn fetch_course_grades(&self, course_id: u64) {
        let token = self.token.clone();
        let userid = self.userid;
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let client = MoodleClient::new(token);
            match client.grades(userid, course_id).await {
                Ok(r) => { let _ = tx.send(AppMsg::GradesDetailLoaded(r.usergrades)); }
                Err(e) => eprintln!("grades detail {course_id}: {e}"),
            }
        });
    }

    fn fetch_profile(&self) {
        let token = self.token.clone();
        let userid = self.userid;
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let client = MoodleClient::new(token);
            match client.user_profile(userid).await {
                Ok(u) => { let _ = tx.send(AppMsg::ProfileLoaded(u)); }
                Err(e) => eprintln!("profile: {e}"),
            }
        });
    }

    fn fetch_course_content(&self, course_id: u64) {
        let token = self.token.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let client = MoodleClient::new(token);
            match client.course_contents(course_id).await {
                Ok(sections) => { let _ = tx.send(AppMsg::CourseContentLoaded { course_id, sections }); }
                Err(e) => eprintln!("course content {course_id}: {e}"),
            }
        });
    }

    fn find_module_info(&self, module_id: u64) -> Option<(String, String)> {
        for section in &self.course_content.sections {
            for module in &section.modules {
                if module.id == module_id {
                    return Some((module.name.clone(), section.name.clone()));
                }
            }
        }
        None
    }

    fn record_activity(&mut self, module_id: u64, action: &str) {
        if let Some((module_name, section_name)) = self.find_module_info(module_id) {
            let course_name = self.courses.courses.iter()
                .find(|c| c.id == self.course_content.course_id)
                .map(|c| c.shortname.clone())
                .unwrap_or_default();
            let entry = ActivityEntry {
                course_id: self.course_content.course_id,
                course_name,
                module_id,
                module_name,
                section_name,
                action: action.to_string(),
                timestamp: chrono::Utc::now().timestamp(),
            };
            let _ = self.storage.record_activity(&entry);
            if let Ok(ra) = self.storage.recent_activity(5) {
                self.courses.recent_activity = ra;
            }
        }
    }

    fn download_file(&self, module_id: u64, url: String, save_path: PathBuf) {
        let token = self.token.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let sep = if url.contains('?') { '&' } else { '?' };
            let full_url = format!("{url}{sep}token={token}");
            let result = async {
                let bytes = reqwest::get(&full_url).await?.bytes().await?;
                if let Some(parent) = save_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&save_path, &bytes)?;
                Ok::<_, anyhow::Error>(save_path.clone())
            }.await;
            match result {
                Ok(path) => { let _ = tx.send(AppMsg::FileDownloaded { module_id, path }); }
                Err(e) => { let _ = tx.send(AppMsg::FileDownloadFailed { module_id, error: e.to_string() }); }
            }
        });
    }

    fn flush_course_timer(&mut self) {
        if let (Some(id), Some(since)) = (self.focused_course, self.focus_since.take()) {
            let secs = since.elapsed().as_secs();
            self.storage.add_course_time(id, secs).ok();
            if let Ok(m) = self.storage.get_course_metrics(id) {
                self.courses.metrics.insert(id, m);
            }
        }
        self.focused_course = None;
    }

    fn on_course_event(&mut self, ev: CoursesEvent) {
        match ev {
            CoursesEvent::Selected(id) => {
                self.flush_course_timer();
                self.focused_course = Some(id);
                self.focus_since = Some(Instant::now());
                self.storage.record_course_open(id).ok();
                if let Ok(m) = self.storage.get_course_metrics(id) {
                    self.courses.metrics.insert(id, m);
                }
                
                // Load course content
                self.course_content.course_id = id;
                self.course_content.course_shortname = self.courses.courses.iter()
                    .find(|c| c.id == id)
                    .map(|c| c.shortname.clone())
                    .unwrap_or_default();
                self.course_content.sections.clear();
                self.course_content.loading = true;
                self.course_content.needs_scroll = true;
                self.course_content.download_states.clear();
                // Load recent changes for What's New panel, then mark seen
                if let Ok(rc) = self.storage.recent_changes(id, 20) {
                    self.course_content.recent_changes = rc;
                    self.course_content.show_changes = !self.course_content.recent_changes.is_empty();
                }
                let _ = self.storage.mark_changes_seen(id);
                self.courses.change_counts.remove(&id);
                // Load from cache first (stale-while-revalidate)
                if let Ok(Some(json)) = self.storage.load_cache(&format!("course_content_{id}")) {
                    if let Ok(v) = serde_json::from_str::<Vec<crate::api::types::CourseSection>>(&json) {
                        self.course_content.sections = v;
                        self.course_content.loading = false;
                    }
                }
                self.fetch_course_content(id);
            }
            CoursesEvent::Deselected => {
                self.flush_course_timer();
                self.course_content.course_id = 0;
                self.course_content.sections.clear();
            }
        }
    }

    fn do_logout(&mut self) {
        self.flush_course_timer();
        self.storage.clear_session().ok();
        self.storage.clear_cache().ok();
        self.storage.clear_telemetry().ok();
        self.token.clear();
        self.private_token.clear();
        self.userid = 0;
        self.fullname.clear();
        self.student_year = None;
        self.courses = CoursesScreen::default();
        self.assignments = AssignmentsScreen::default();
        self.assignment_detail = AssignmentDetailScreen::default();
        self.calendar = CalendarScreen::default();
        self.notifications = NotificationsScreen::default();
        self.grades = GradesScreen::default();
        self.profile = ProfileScreen::default();
        self.login = LoginScreen::default();
        self.active_tab = Tab::Courses;
        self.assignments_loaded = false;
        self.calendar_loaded = false;
        self.notifications_loaded = false;
        self.grades_loaded = false;
        self.profile_loaded = false;
        self.screen = Screen::Login;
    }

    fn show_tab_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("tab_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                for &tab in TABS {
                    let label = if tab == Tab::Notifications && self.notifications.unread_count > 0 {
                        format!("{} ({})", tab.label(), self.notifications.unread_count)
                    } else {
                        tab.label().to_string()
                    };
                    let selected = self.active_tab == tab;
                    let color = if tab == Tab::Notifications && self.notifications.unread_count > 0 {
                        Some(egui::Color32::from_rgb(255, 180, 50))
                    } else { None };
                    let text = if let Some(c) = color {
                        egui::RichText::new(label).size(14.0).color(c)
                    } else {
                        egui::RichText::new(label).size(14.0)
                    };
                    if ui.selectable_label(selected, text).clicked() {
                        if self.active_tab == Tab::Courses && tab != Tab::Courses {
                            self.flush_course_timer();
                            self.courses.selected_course = None;
                        }
                        self.active_tab = tab;
                        match tab {
                            Tab::Announcements if !self.announcements_loaded => {
                                self.announcements_loaded = true;
                                self.fetch_announcements();
                            }
                            Tab::Assignments if !self.assignments_loaded => {
                                self.assignments_loaded = true;
                                self.fetch_assignments();
                            }
                            Tab::Calendar if !self.calendar_loaded => {
                                self.calendar_loaded = true;
                                self.fetch_calendar();
                            }
                            Tab::Notifications if !self.notifications_loaded => {
                                self.notifications_loaded = true;
                                self.fetch_notifications();
                            }
                            Tab::Grades if !self.grades_loaded => {
                                self.grades_loaded = true;
                                self.fetch_grades_overview();
                            }
                            Tab::Profile if !self.profile_loaded => {
                                self.profile_loaded = true;
                                self.fetch_profile();
                            }
                            _ => {}
                        }
                    }
                    ui.separator();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(egui::RichText::new("Logout").size(13.0)).clicked() {
                        self.do_logout();
                    }
                    if ui.button(egui::RichText::new(egui_phosphor::regular::GEAR).size(16.0)).clicked() {
                        self.show_settings = !self.show_settings;
                    }

                    ui.separator();
                    if let Some(year) = self.student_year {
                        ui.label(egui::RichText::new(year_label(year)).size(12.0)
                            .color(egui::Color32::from_rgb(100, 160, 220)));
                        ui.separator();
                    }
                    if !self.fullname.is_empty() {
                        ui.label(egui::RichText::new(&self.fullname).size(13.0)
                            .color(ui.visuals().weak_text_color()));
                    }
                });
            });
        });
    }

    fn show_settings_panel(&mut self, ctx: &egui::Context) {
        if !self.show_settings { return; }
        egui::Window::new("Settings")
            .collapsible(false)
            .resizable(false)
            .default_width(300.0)
            .anchor(egui::Align2::RIGHT_TOP, [-10.0, 40.0])
            .show(ctx, |ui| {
                ui.label(egui::RichText::new("Background Notifications").strong());
                ui.separator();
                ui.add_space(4.0);

                let prev = self.bg_enabled;
                ui.checkbox(&mut self.bg_enabled, "Run notification daemon on login");
                if self.bg_enabled != prev {
                    if self.bg_enabled {
                        if background::create_autostart().is_ok() {
                            let tx_bg: std::sync::mpsc::Sender<u64> = {
                                let (s, r) = std::sync::mpsc::channel::<u64>();
                                let tx = self.tx.clone();
                                std::thread::spawn(move || {
                                    for count in r { let _ = tx.send(AppMsg::NewNotifications(count)); }
                                });
                                s
                            };
                            background::spawn_poller(self.token.clone(), self.userid, tx_bg);
                        }
                    } else {
                        background::remove_autostart();
                    }
                }

                ui.add_space(6.0);
                ui.label(egui::RichText::new(
                    "Polls notifications every 10 min and course content changes every 30 min. Sends desktop notifications for new activity and updated course materials.")
                    .size(11.0).color(ui.visuals().weak_text_color()));

                ui.add_space(10.0);
                if ui.button("Close").clicked() { self.show_settings = false; }
            });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                AppMsg::LoginOk { token, private_token, info } => {
                    self.storage.set("token", &token).ok();
                    self.storage.set("private_token", &private_token).ok();
                    self.storage.set("userid", &info.userid.to_string()).ok();
                    self.storage.set("fullname", &info.fullname).ok();
                    self.storage.set("username", &self.login.username).ok();
                    let username = self.login.username.clone();
                    let password = self.login.password.clone();
                    tokio::spawn(async move {
                        if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, &username) {
                            entry.set_password(&password).ok();
                        }
                    });
                    self.token = token.clone();
                    self.private_token = private_token.clone();
                    self.userid = info.userid;
                    self.fullname = info.fullname;
                    self.login.loading = false;
                    self.screen = Screen::Main;
                    let tx = self.tx.clone();
                    let t2 = token.clone();
                    let uid = self.userid;
                    tokio::spawn(async move {
                        if let Ok(c) = MoodleClient::new(t2).enrolled_courses(uid).await {
                            let _ = tx.send(AppMsg::CoursesLoaded(c));
                        }
                    });
                    ctx.request_repaint();
                }
                AppMsg::LoginErr(err) => {
                    self.login.error = Some(err);
                    self.login.loading = false;
                    ctx.request_repaint();
                }
                AppMsg::CoursesLoaded(courses) => {
                    self.student_year = infer_student_year(&courses);
                    for c in &courses {
                        if let Ok(m) = self.storage.get_course_metrics(c.id) {
                            if m.0 > 0 || m.1 > 0 { self.courses.metrics.insert(c.id, m); }
                        }
                    }
                    // Populate grades course list for overview display
                    self.grades.course_list = courses.iter()
                        .map(|c| (c.id, c.fullname.clone(), c.shortname.clone()))
                        .collect();
                    // Populate profile dept breakdown
                    let mut dept_map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
                    for c in &courses {
                        if let Some(d) = parse_dept(&c.shortname) {
                            *dept_map.entry(d).or_insert(0) += 1;
                        }
                    }
                    let mut dept_counts: Vec<(String, usize)> = dept_map.into_iter().collect();
                    dept_counts.sort_by_key(|(d, _)| d.clone());
                    self.profile.dept_counts = dept_counts;
                    self.profile.student_year = self.student_year;
                    self.courses.courses = courses;
                    self.apply_year_defaults();
                    if let Ok(json) = serde_json::to_string(&self.courses.courses) {
                        let _ = self.storage.save_cache("courses", &json);
                    }
                    ctx.request_repaint();
                }
                AppMsg::CourseContentLoaded { course_id, sections } => {
                    // Diff against fingerprints to detect changes
                    if let Ok(stored) = self.storage.load_fingerprints(course_id) {
                        let (changes, new_fps, removed_ids) = crate::telemetry::diff_content(course_id, &sections, &stored);
                        if !changes.is_empty() {
                            let _ = self.storage.save_changes(&changes);
                            if let Ok(counts) = self.storage.unseen_change_counts() {
                                self.courses.change_counts = counts;
                            }
                            // If currently viewing this course, refresh What's New panel
                            if self.course_content.course_id == course_id {
                                if let Ok(rc) = self.storage.recent_changes(course_id, 20) {
                                    self.course_content.recent_changes = rc;
                                    self.course_content.show_changes = true;
                                }
                                // Re-mark seen since user is actively looking at it
                                let _ = self.storage.mark_changes_seen(course_id);
                                self.courses.change_counts.remove(&course_id);
                            }
                        }
                        let _ = self.storage.upsert_fingerprints(course_id, &new_fps);
                        let _ = self.storage.delete_fingerprints(&removed_ids);
                    }

                    if self.course_content.course_id == course_id {
                        if let Ok(json) = serde_json::to_string(&sections) {
                            let _ = self.storage.save_cache(&format!("course_content_{course_id}"), &json);
                        }
                        self.course_content.sections = sections;
                        self.course_content.loading = false;
                    }
                    ctx.request_repaint();
                }
                AppMsg::AnnouncementsLoaded(ann) => {
                    self.announcements.announcements = ann;
                    if let Ok(json) = serde_json::to_string(&self.announcements.announcements) {
                        let _ = self.storage.save_cache("announcements", &json);
                    }
                    // Feed announcements into calendar
                    self.calendar.announcement_events = self.announcements.announcements.iter()
                        .map(|a| CalendarEvent {
                            id: a.discussion.id,
                            name: a.discussion.name.clone(),
                            description: Some(a.discussion.message.clone()),
                            timestart: a.discussion.timecreated,
                            timesort: a.discussion.timecreated,
                            courseid: a.course_id,
                            coursename: Some(a.course_name.clone()),
                            modulename: Some("announcement".into()),
                            eventtype: Some("announcement".into()),
                        }).collect();
                    ctx.request_repaint();
                }
                AppMsg::AssignmentsLoaded(r) => {
                    self.assignments.courses = r.courses;
                    if let Ok(json) = serde_json::to_string(&self.assignments.courses) {
                        let _ = self.storage.save_cache("assignments", &json);
                    }
                    // Feed assignment due dates into calendar (full replace)
                    self.calendar.assignment_events = self.assignments.courses.iter()
                        .flat_map(|c| c.assignments.iter().filter(|a| a.duedate > 0).map(move |a| CalendarEvent {
                            id: a.id,
                            name: format!("{}: {}", c.shortname, a.name),
                            description: a.intro.clone(),
                            timestart: a.duedate,
                            timesort: a.duedate,
                            courseid: c.id,
                            coursename: Some(c.fullname.clone()),
                            modulename: Some("assign".into()),
                            eventtype: Some("due".into()),
                        }))
                        .collect();
                    ctx.request_repaint();
                }
                AppMsg::AssignmentStatusLoaded { assign_id, status } => {
                    self.assignments.loading_statuses.remove(&assign_id);
                    if self.assignment_detail.assignment.as_ref().map(|a| a.id) == Some(assign_id) {
                        self.assignment_detail.status = Some(status.clone());
                        self.assignment_detail.loading_status = false;
                    }
                    self.assignments.submission_statuses.insert(assign_id, status);
                    ctx.request_repaint();
                }
                AppMsg::CalendarLoaded(r) => {
                    self.calendar.events = r.events;
                    if let Ok(json) = serde_json::to_string(&self.calendar.events) {
                        let _ = self.storage.save_cache("calendar", &json);
                    }
                    ctx.request_repaint();
                }
                AppMsg::NotificationsLoaded(r) => {
                    self.notifications.unread_count = r.unreadcount;
                    self.notifications.notifications = r.notifications;
                    if let Ok(json) = serde_json::to_string(&self.notifications.notifications) {
                        let _ = self.storage.save_cache("notifications", &json);
                    }
                    ctx.request_repaint();
                }
                AppMsg::GradeOverviewLoaded(r) => {
                    self.grades.overview.clear();
                    for item in r.grades {
                        if let Some(g) = item.grade {
                            if !g.is_empty() {
                                self.grades.overview.insert(item.courseid, g);
                            }
                        }
                    }
                    ctx.request_repaint();
                }
                AppMsg::GradesDetailLoaded(detail) => {
                    for ug in detail {
                        self.grades.detail_loading.remove(&ug.courseid);
                        if let Some(existing) = self.grades.grades.iter_mut().find(|g| g.courseid == ug.courseid) {
                            *existing = ug;
                        } else {
                            self.grades.grades.push(ug);
                        }
                    }
                    if let Ok(json) = serde_json::to_string(&self.grades.grades) {
                        let _ = self.storage.save_cache("grades", &json);
                    }
                    ctx.request_repaint();
                }
                AppMsg::ProfileLoaded(user) => {
                    self.profile.user = Some(user);
                    self.profile.student_year = self.student_year;
                    ctx.request_repaint();
                }
                AppMsg::OpenUrl(url) => {
                    let private_token = self.private_token.clone();
                    let token = self.token.clone();
                    tokio::spawn(async move {
                        fn pct_encode(s: &str) -> String {
                            s.bytes().map(|b| match b {
                                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'
                                | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
                                b => format!("%{:02X}", b),
                            }).collect()
                        }

                        // Protected file: append wstoken for direct download
                        if url.contains("/pluginfile.php") {
                            let sep = if url.contains('?') { '&' } else { '?' };
                            let _ = open::that(format!("{url}{sep}token={token}"));
                            return;
                        }

                        // Try autologin if private_token available
                        if !private_token.is_empty() {
                            let client = MoodleClient::new(token);
                            if let Ok(resp) = client.get_autologin_url(&private_token).await {
                                let _ = open::that(
                                    format!("{}&url={}", resp.autologinurl, pct_encode(&url)));
                                return;
                            }
                            eprintln!("autologin API failed, using wantsurl fallback");
                        }

                        // Open directly — Moodle redirects to login+back automatically when
                        // session is missing. wantsurl causes "already logged in" confirm dialog.
                        let _ = open::that(&url);
                    });
                }
                AppMsg::FileDownloaded { module_id, path } => {
                    self.course_content.download_states.insert(module_id, DownloadState::Done(path.clone()));
                    self.record_activity(module_id, "downloaded");
                    let _ = open::that(&path);
                    ctx.request_repaint();
                }
                AppMsg::FileDownloadFailed { module_id, error } => {
                    self.course_content.download_states.insert(module_id, DownloadState::Error(error));
                    ctx.request_repaint();
                }
                AppMsg::NewNotifications(count) => {
                    self.notifications.unread_count += count;
                    ctx.request_repaint();
                }
                AppMsg::FilePicked { assign_id, filename, data } => {
                    if self.assignment_detail.assignment.as_ref().map(|a| a.id) == Some(assign_id) {
                        self.assignment_detail.pending_file = Some((filename, data));
                        self.assignment_detail.upload_state = UploadState::Idle;
                    }
                    ctx.request_repaint();
                }
                AppMsg::AssignmentUploadDone { assign_id } => {
                    if self.assignment_detail.assignment.as_ref().map(|a| a.id) == Some(assign_id) {
                        self.assignment_detail.upload_state = UploadState::Done;
                        self.assignment_detail.pending_file = None;
                        self.assignment_detail.loading_status = true;
                        self.fetch_assignment_status(assign_id);
                    }
                    self.assignments.submission_statuses.remove(&assign_id);
                    ctx.request_repaint();
                }
                AppMsg::AssignmentUploadFailed { assign_id, error } => {
                    if self.assignment_detail.assignment.as_ref().map(|a| a.id) == Some(assign_id) {
                        self.assignment_detail.upload_state = UploadState::Error(error);
                    }
                    ctx.request_repaint();
                }
                AppMsg::TokenExpired => {
                    self.do_logout();
                    ctx.request_repaint();
                }
            }
        }

        // ── Status bar ───────────────────────────────────────────────────
        if matches!(self.screen, Screen::Main) {
            egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let now = chrono::Utc::now().timestamp();
                    ui.colored_label(egui::Color32::from_rgb(80, 200, 80), format!("{} Online", egui_phosphor::regular::CHECK_CIRCLE));
                    ui.separator();

                    if self.notifications.unread_count > 0 {
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 180, 50),
                            format!("{} {} unread", egui_phosphor::regular::BELL, self.notifications.unread_count),
                        );
                        ui.separator();
                    }

                    let upcoming: Vec<_> = self.assignments.courses.iter()
                        .flat_map(|c| c.assignments.iter())
                        .filter(|a| a.duedate > now)
                        .collect();
                    if !upcoming.is_empty() {
                        ui.label(egui::RichText::new(format!("{} {} upcoming", egui_phosphor::regular::FILE_TEXT, upcoming.len()))
                            .size(12.0).color(ui.visuals().weak_text_color()));
                        ui.separator();
                    }

                    let mut soonest = upcoming.clone();
                    soonest.sort_by_key(|a| a.duedate);
                    if let Some(next) = soonest.first() {
                        let diff = next.duedate - now;
                        let time_str = if diff < 3600 {
                            format!("{} min", diff / 60)
                        } else if diff < 86400 {
                            format!("{:.0}h", diff as f64 / 3600.0)
                        } else {
                            format!("{:.0}d", diff as f64 / 86400.0)
                        };
                        let color = if diff < 86400 {
                            egui::Color32::from_rgb(220, 100, 60)
                        } else {
                            ui.visuals().weak_text_color()
                        };
                        let short: String = next.name.chars().take(30).collect();
                        ui.label(egui::RichText::new(format!("{} Next: {} — {}", egui_phosphor::regular::CLOCK, short, time_str))
                            .size(12.0).color(color));
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(
                            chrono::Local::now().format("%H:%M").to_string()
                        ).size(11.0).color(ui.visuals().weak_text_color()));
                    });
                });
            });
        }

        match self.screen {
            Screen::Login => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let submit = self.login.show(ui);
                    if submit && !self.login.loading {
                        let username = self.login.username.clone();
                        let password = self.login.password.clone();
                        let tx = self.tx.clone();
                        self.login.loading = true;
                        self.login.error = None;
                        tokio::spawn(async move {
                            match MoodleClient::login(&username, &password).await {
                                Ok((token, private_token, info)) => { let _ = tx.send(AppMsg::LoginOk { token, private_token, info }); }
                                Err(e) => { let _ = tx.send(AppMsg::LoginErr(e.to_string())); }
                            }
                        });
                    }
                });
            }
            Screen::Main => {
                self.show_tab_bar(ctx);
                self.show_settings_panel(ctx);
                egui::CentralPanel::default().show(ctx, |ui| {
                    match self.active_tab {
                        Tab::Courses => {
                            if self.assignment_detail.assignment.is_some()
                                && self.assignment_detail.source == DetailSource::CourseContent {
                                if let Some(ev) = self.assignment_detail.show(ui) {
                                    self.handle_detail_event(ev);
                                }
                            } else if let Some(selected_id) = self.courses.selected_course {
                                let name = self.courses.courses.iter()
                                    .find(|c| c.id == selected_id)
                                    .map(|c| c.fullname.clone())
                                    .unwrap_or_else(|| "Course".into());
                                
                                egui::TopBottomPanel::top("course_content_top").show_inside(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        if ui.button("⬅ Back").clicked() {
                                            self.on_course_event(CoursesEvent::Deselected);
                                            self.courses.selected_course = None;
                                        }
                                        ui.label(egui::RichText::new(decode_html(&name)).size(16.0).strong());
                                    });
                                });
                                egui::CentralPanel::default().show_inside(ui, |ui| {
                                    match self.course_content.show(ui) {
                                        Some(CourseContentEvent::OpenUrl { url, module_id }) => {
                                            self.record_activity(module_id, "opened");
                                            let _ = self.tx.send(AppMsg::OpenUrl(url));
                                        }
                                        Some(CourseContentEvent::Download { module_id, url, save_path }) => {
                                            self.course_content.download_states.insert(module_id, DownloadState::Downloading);
                                            self.download_file(module_id, url, save_path);
                                        }
                                        Some(CourseContentEvent::OpenFile(path)) => {
                                            // Find module_id by matching path in download_states
                                            let mid = self.course_content.download_states.iter()
                                                .find_map(|(id, ds)| match ds {
                                                    DownloadState::Done(p) if *p == path => Some(*id),
                                                    _ => None,
                                                });
                                            if let Some(mid) = mid { self.record_activity(mid, "opened"); }
                                            let _ = open::that(&path);
                                        }
                                        Some(CourseContentEvent::OpenAssignment { cmid }) => {
                                            let found = self.assignments.courses.iter()
                                                .flat_map(|c| c.assignments.iter().map(move |a| (a, c.fullname.clone())))
                                                .find(|(a, _)| a.cmid == cmid)
                                                .map(|(a, cn)| (a.clone(), cn));
                                            if let Some((assign, course_name)) = found {
                                                // Stay on Courses tab — Back returns to course content
                                                self.open_assignment_detail(assign, course_name, DetailSource::CourseContent);
                                            } else {
                                                if !self.assignments_loaded {
                                                    self.assignments_loaded = true;
                                                    self.fetch_assignments();
                                                }
                                                let url = format!("https://sci.cmb.ac.lk/lms/mod/assign/view.php?id={cmid}");
                                                let _ = self.tx.send(AppMsg::OpenUrl(url));
                                            }
                                        }
                                        Some(CourseContentEvent::ShowFolder(path)) => {
                                            let folder = path.parent()
                                                .map(|p| p.to_path_buf())
                                                .unwrap_or(path);
                                            let _ = open::that(&folder);
                                        }
                                        None => {}
                                    }
                                });
                            } else {
                                if let Some(ev) = self.courses.show(ui) { self.on_course_event(ev); }
                            }
                        }
                        Tab::Announcements => { self.announcements.show(ui); }
                        Tab::Assignments  => {
                            use crate::screens::assignments::AssignmentsEvent;
                            if self.assignment_detail.assignment.is_some()
                                && self.assignment_detail.source == DetailSource::Assignments {
                                if let Some(ev) = self.assignment_detail.show(ui) {
                                    self.handle_detail_event(ev);
                                }
                            } else if let Some(ev) = self.assignments.show(ui) {
                                match ev {
                                    AssignmentsEvent::RequestStatus(id) => {
                                        self.assignments.loading_statuses.insert(id);
                                        self.fetch_assignment_status(id);
                                    }
                                    AssignmentsEvent::OpenDetail { assign, course_name } => {
                                        self.open_assignment_detail(assign, course_name, DetailSource::Assignments);
                                    }
                                }
                            }
                        }
                        Tab::Calendar     => {
                            use crate::screens::calendar::CalendarScreenEvent;
                            if let Some(ev) = self.calendar.show(ui) {
                                match ev {
                                    CalendarScreenEvent::DeletePersonal(id) => {
                                        self.calendar.personal_events.retain(|e| e.id != id);
                                    }
                                }
                            }
                        }
                        Tab::Notifications => {
                            use crate::screens::notifications::NotificationsEvent;
                            if let Some(NotificationsEvent::OpenUrl(url)) = self.notifications.show(ui) {
                                let _ = self.tx.send(AppMsg::OpenUrl(url));
                            }
                        }
                        Tab::Grades => {
                            if let Some(cid) = self.grades.show(ui) {
                                if !self.grades.detail_loading.contains(&cid) {
                                    self.grades.detail_loading.insert(cid);
                                    self.fetch_course_grades(cid);
                                }
                            }
                        }
                        Tab::Profile => { self.profile.show(ui); }
                    }
                });
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.flush_course_timer();
    }
}

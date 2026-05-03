use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Instant;
use crate::api::{is_token_error, types::*, MoodleClient};
use crate::background;
use crate::models::{infer_student_year, parse_dept, year_label};
use crate::screens::{
    assignments::AssignmentsScreen,
    calendar::CalendarScreen,
    courses::{CoursesEvent, CoursesScreen},
    grades::GradesScreen,
    login::LoginScreen,
    notifications::NotificationsScreen,
    profile::ProfileScreen,
};
use crate::storage::Storage;

const KEYRING_SERVICE: &str = "campus-lms";

#[derive(PartialEq, Clone, Copy)]
enum Tab { Courses, Assignments, Calendar, Notifications, Grades, Profile }

impl Tab {
    fn label(self) -> &'static str {
        match self {
            Tab::Courses       => "Courses",
            Tab::Assignments   => "Assignments",
            Tab::Calendar      => "Calendar",
            Tab::Notifications => "Notifications",
            Tab::Grades        => "Grades",
            Tab::Profile       => "Profile",
        }
    }
}
const TABS: &[Tab] = &[
    Tab::Courses, Tab::Assignments, Tab::Calendar,
    Tab::Notifications, Tab::Grades, Tab::Profile,
];

enum Screen { Login, Main }

enum AppMsg {
    LoginOk { token: String, info: SiteInfo },
    LoginErr(String),
    CoursesLoaded(Vec<Course>),
    AssignmentsLoaded(AssignmentsResponse),
    CalendarLoaded(CalendarEventList),
    NotificationsLoaded(NotificationList),
    GradeOverviewLoaded(GradeOverviewResponse),
    GradesDetailLoaded(Vec<UserGrades>),
    ProfileLoaded(UserProfile),
    TokenExpired,
    NewNotifications(u64),
}

pub struct App {
    screen: Screen,
    active_tab: Tab,
    login: LoginScreen,
    courses: CoursesScreen,
    assignments: AssignmentsScreen,
    calendar: CalendarScreen,
    notifications: NotificationsScreen,
    grades: GradesScreen,
    profile: ProfileScreen,
    storage: Storage,
    token: String,
    userid: u64,
    fullname: String,
    student_year: Option<u8>,
    // Time tracking
    focused_course: Option<u64>,
    focus_since: Option<Instant>,
    // Data loaded flags
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

        let bg_enabled = background::autostart_enabled();

        let mut app = Self {
            screen: Screen::Login,
            active_tab: Tab::Courses,
            login: LoginScreen::default(),
            courses: CoursesScreen::default(),
            assignments: AssignmentsScreen::default(),
            calendar: CalendarScreen::default(),
            notifications: NotificationsScreen::default(),
            grades: GradesScreen::default(),
            profile: ProfileScreen::default(),
            storage,
            token: String::new(),
            userid: 0,
            fullname: String::new(),
            student_year: None,
            focused_course: None,
            focus_since: None,
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
                app.userid = userid;
                app.fullname = app.storage.get("fullname").ok().flatten().unwrap_or_default();
                app.screen = Screen::Main;
                let tx = app.tx.clone();
                let username = app.storage.get("username").ok().flatten().unwrap_or_default();
                tokio::spawn(async move {
                    let client = MoodleClient::new(token.clone());
                    match client.enrolled_courses(userid).await {
                        Ok(courses) => { let _ = tx.send(AppMsg::CoursesLoaded(courses)); }
                        Err(e) if is_token_error(&e) => {
                            if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, &username) {
                                if let Ok(password) = entry.get_password() {
                                    match MoodleClient::login(&username, &password).await {
                                        Ok((t, info)) => { let _ = tx.send(AppMsg::LoginOk { token: t, info }); }
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
            if self.assignments.year_filter.is_none() { self.assignments.year_filter = Some(y); }
            if self.grades.year_filter.is_none()      { self.grades.year_filter      = Some(y); }
        }
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
            }
            CoursesEvent::Deselected => self.flush_course_timer(),
        }
    }

    fn do_logout(&mut self) {
        self.flush_course_timer();
        self.storage.clear_session().ok();
        self.token.clear();
        self.userid = 0;
        self.fullname.clear();
        self.student_year = None;
        self.courses = CoursesScreen::default();
        self.assignments = AssignmentsScreen::default();
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
                    if ui.button(egui::RichText::new("⚙").size(14.0)).clicked() {
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
                ui.label(egui::RichText::new("When enabled, Campus LMS starts in background on login and sends desktop notifications for new activity.").size(11.0)
                    .color(ui.visuals().weak_text_color()));

                ui.add_space(10.0);
                if ui.button("Close").clicked() { self.show_settings = false; }
            });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                AppMsg::LoginOk { token, info } => {
                    self.storage.set("token", &token).ok();
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
                    ctx.request_repaint();
                }
                AppMsg::AssignmentsLoaded(r) => {
                    self.assignments.courses = r.courses;
                    ctx.request_repaint();
                }
                AppMsg::CalendarLoaded(r) => {
                    self.calendar.events = r.events;
                    ctx.request_repaint();
                }
                AppMsg::NotificationsLoaded(r) => {
                    self.notifications.unread_count = r.unreadcount;
                    self.notifications.notifications = r.notifications;
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
                        // Replace existing or push new
                        if let Some(existing) = self.grades.grades.iter_mut().find(|g| g.courseid == ug.courseid) {
                            *existing = ug;
                        } else {
                            self.grades.grades.push(ug);
                        }
                    }
                    ctx.request_repaint();
                }
                AppMsg::ProfileLoaded(user) => {
                    self.profile.user = Some(user);
                    self.profile.student_year = self.student_year;
                    ctx.request_repaint();
                }
                AppMsg::TokenExpired => { self.do_logout(); ctx.request_repaint(); }
                AppMsg::NewNotifications(count) => {
                    self.notifications.unread_count += count;
                    ctx.request_repaint();
                }
            }
        }

        // ── Status bar ───────────────────────────────────────────────────
        if matches!(self.screen, Screen::Main) {
            egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let now = chrono::Utc::now().timestamp();
                    ui.colored_label(egui::Color32::from_rgb(80, 200, 80), "● Online");
                    ui.separator();

                    if self.notifications.unread_count > 0 {
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 180, 50),
                            format!("🔔 {} unread", self.notifications.unread_count),
                        );
                        ui.separator();
                    }

                    let upcoming: Vec<_> = self.assignments.courses.iter()
                        .flat_map(|c| c.assignments.iter())
                        .filter(|a| a.duedate > now)
                        .collect();
                    if !upcoming.is_empty() {
                        ui.label(egui::RichText::new(format!("📄 {} upcoming", upcoming.len()))
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
                        ui.label(egui::RichText::new(format!("⏰ Next: {short} — {time_str}"))
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
                                Ok((token, info)) => { let _ = tx.send(AppMsg::LoginOk { token, info }); }
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
                            if let Some(ev) = self.courses.show(ui) { self.on_course_event(ev); }
                        }
                        Tab::Assignments  => self.assignments.show(ui),
                        Tab::Calendar     => self.calendar.show(ui),
                        Tab::Notifications => self.notifications.show(ui),
                        Tab::Grades => {
                            if let Some(cid) = self.grades.show(ui) {
                                if !self.grades.detail_loading.contains(&cid) {
                                    self.grades.detail_loading.insert(cid);
                                    self.fetch_course_grades(cid);
                                }
                            }
                        }
                        Tab::Profile => self.profile.show(ui),
                    }
                });
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.flush_course_timer();
    }
}

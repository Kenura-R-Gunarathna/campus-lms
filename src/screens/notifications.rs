use egui::Ui;
use chrono::Utc;
use crate::api::types::MoodleNotification;

fn is_relevant(n: &MoodleNotification, now: i64) -> bool {
    if !n.is_read { return true; }
    let age_days = (now - n.timecreated).max(0) / 86400;
    let s = n.subject.to_lowercase();

    if s.contains("upcoming activities") || s.contains("upcoming events") {
        return age_days < 3;
    }
    if s.contains("submitted") || s.contains("submission") {
        return age_days < 90;
    }
    if s.contains("sign in") || s.contains("new sign") {
        return age_days < 7;
    }
    if s.contains("grade") || s.contains("graded") || s.contains("feedback") {
        return age_days < 60;
    }
    if s.contains("message") || s.contains("reply") {
        return age_days < 30;
    }
    age_days < 30
}

fn fmt_age(ts: i64, now: i64) -> String {
    let secs = now - ts;
    if secs < 60 { return "just now".into(); }
    let m = secs / 60;
    let h = m / 60;
    let d = h / 24;
    if d > 0 { format!("{d}d ago") }
    else if h > 0 { format!("{h}h ago") }
    else { format!("{m}m ago") }
}

fn notif_icon(subject: &str) -> (&'static str, &'static str) {
    let s = subject.to_lowercase();
    if s.contains("submitted") || s.contains("submission") { (egui_phosphor::regular::FILE_TEXT, "Submission") }
    else if s.contains("upcoming") || s.contains("due") { (egui_phosphor::regular::CLOCK, "Upcoming Alert") }
    else if s.contains("grade") || s.contains("graded") { (egui_phosphor::regular::GRADUATION_CAP, "Grade Update") }
    else if s.contains("sign in") { (egui_phosphor::regular::SIGN_IN, "Security Alert") }
    else if s.contains("message") { (egui_phosphor::regular::CHAT_CIRCLE, "Message") }
    else { (egui_phosphor::regular::BELL, "Notification") }
}

pub struct NotificationsScreen {
    pub notifications: Vec<MoodleNotification>,
    pub unread_count: u64,
    pub show_archived: bool,
}

impl Default for NotificationsScreen {
    fn default() -> Self {
        Self { notifications: vec![], unread_count: 0, show_archived: false }
    }
}

impl NotificationsScreen {
    pub fn show(&mut self, ui: &mut Ui) {
        let now = Utc::now().timestamp();

        egui::TopBottomPanel::top("notif_topbar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Notifications").size(16.0).strong());
                if self.unread_count > 0 {
                    ui.label(
                        egui::RichText::new(format!("  {} unread", self.unread_count))
                            .size(13.0)
                            .color(egui::Color32::from_rgb(255, 180, 50)),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut self.show_archived, "Show old");
                });
            });
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if self.notifications.is_empty() {
                ui.centered_and_justified(|ui| { ui.spinner(); });
                return;
            }

            let visible: Vec<&MoodleNotification> = self.notifications.iter()
                .filter(|n| self.show_archived || is_relevant(n, now))
                .collect();

            if visible.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(30.0);
                    ui.label(egui::RichText::new("No relevant notifications")
                        .color(ui.visuals().weak_text_color()));
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Enable \"Show old\" to see archived ones")
                        .size(12.0).color(ui.visuals().weak_text_color()));
                });
                return;
            }

            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for notif in visible {
                    ui.add_space(4.0);
                    let (icon, tooltip) = notif_icon(&notif.subject);
                    let age_str = fmt_age(notif.timecreated, now);

                    let bg = if !notif.is_read {
                        egui::Color32::from_rgba_unmultiplied(40, 60, 100, 80)
                    } else {
                        ui.visuals().faint_bg_color
                    };

                    egui::Frame::none()
                        .fill(bg)
                        .rounding(5.0)
                        .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(icon).size(14.0).color(ui.visuals().weak_text_color()))
                                    .on_hover_text(tooltip);
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(&notif.subject).size(14.0)
                                            .strong_if(!notif.is_read));
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            ui.label(egui::RichText::new(&age_str).size(11.0)
                                                .color(ui.visuals().weak_text_color()));
                                        });
                                    });

                                    // First non-empty line of message
                                    let preview: String = notif.fullmessage.lines()
                                        .map(|l| l.trim())
                                        .find(|l| !l.is_empty())
                                        .unwrap_or("")
                                        .chars().take(120).collect();
                                    if !preview.is_empty() {
                                        ui.label(egui::RichText::new(preview).size(12.0)
                                            .color(ui.visuals().weak_text_color()));
                                    }
                                });
                            });
                        });
                }
            });
        });
    }
}

// Helper trait for conditional strong text
trait RichTextExt {
    fn strong_if(self, cond: bool) -> egui::RichText;
}
impl RichTextExt for egui::RichText {
    fn strong_if(self, cond: bool) -> egui::RichText {
        if cond { self.strong() } else { self }
    }
}

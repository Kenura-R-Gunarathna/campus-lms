use egui::Ui;
use serde::{Deserialize, Serialize};
use crate::api::types::ForumDiscussion;
use crate::models::{decode_html, parse_year, year_label};

#[derive(Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Announcement {
    pub discussion: ForumDiscussion,
    pub course_id: u64,
    pub course_name: String,
}

pub struct AnnouncementsScreen {
    pub announcements: Vec<Announcement>,
    pub student_year: Option<u8>,
    pub search: String,
    pub year_filter: Option<u8>,
    pub my_courses_only: bool,
}

impl Default for AnnouncementsScreen {
    fn default() -> Self {
        Self {
            announcements: vec![],
            student_year: None,
            search: String::new(),
            year_filter: None,
            my_courses_only: true,
        }
    }
}

impl AnnouncementsScreen {
    pub fn show(&mut self, ui: &mut Ui) {
        let now = chrono::Utc::now().timestamp();

        egui::TopBottomPanel::top("ann_topbar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Course Announcements").size(16.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.search)
                        .desired_width(180.0).hint_text("Search..."));
                });
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.my_courses_only, "My courses only");
                if let Some(y) = self.student_year {
                    ui.separator();
                    ui.label(egui::RichText::new(format!("Current: {}", year_label(y)))
                        .size(11.0).color(ui.visuals().weak_text_color()));
                }
            });
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if self.announcements.is_empty() {
                ui.centered_and_justified(|ui| { ui.spinner(); });
                return;
            }

            let search_lower = self.search.to_lowercase();
            let mut filtered: Vec<&Announcement> = self.announcements.iter()
                .filter(|a| {
                    let course_year = parse_year(&a.course_name);
                    let relevance_ok = if self.my_courses_only {
                         self.student_year.map_or(true, |y| course_year == Some(y))
                    } else {
                        true
                    };

                    let search_ok = search_lower.is_empty()
                        || a.discussion.name.to_lowercase().contains(&search_lower)
                        || a.course_name.to_lowercase().contains(&search_lower);

                    relevance_ok && search_ok
                })
                .collect();

            filtered.sort_by_key(|a| -a.discussion.timemodified);

            if filtered.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new("No announcements found").color(ui.visuals().weak_text_color()));
                });
                return;
            }

            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for ann in filtered {
                    render_discussion(ui, ann, now);
                }
            });
        });
    }
}

fn render_discussion(ui: &mut Ui, ann: &Announcement, _now: i64) {
    let disc = &ann.discussion;
    let dt: chrono::DateTime<chrono::Local> =
        chrono::DateTime::from(chrono::DateTime::<chrono::Utc>::from_timestamp(disc.timemodified, 0).unwrap());

    ui.add_space(4.0);
    egui::Frame::none()
        .fill(ui.visuals().faint_bg_color)
        .rounding(6.0)
        .inner_margin(egui::Margin::symmetric(12.0, 10.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(decode_html(&disc.name)).size(14.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                         ui.label(egui::RichText::new(&ann.course_name).size(10.0)
                            .color(ui.visuals().weak_text_color()));
                    });
                });
                
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&disc.userfullname).size(11.0)
                        .color(egui::Color32::from_rgb(100, 160, 220)));
                    ui.label(egui::RichText::new("•").size(11.0).color(ui.visuals().weak_text_color()));
                    ui.label(egui::RichText::new(dt.format("%d %b %Y, %H:%M").to_string()).size(11.0)
                        .color(ui.visuals().weak_text_color()));
                });

                ui.add_space(6.0);
                let msg = decode_html(&disc.message);
                let trimmed = if msg.len() > 200 { format!("{}...", &msg[..200]) } else { msg };
                ui.label(egui::RichText::new(trimmed).size(12.0));

                if disc.numreplies > 0 {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(format!("{} replies", disc.numreplies)).size(10.0)
                        .color(ui.visuals().weak_text_color()));
                }
            });
        });
    ui.add_space(4.0);
}

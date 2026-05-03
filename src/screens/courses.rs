use egui::Ui;
use std::collections::BTreeSet;
use crate::api::types::Course;
use crate::models::{decode_html, parse_dept, parse_year, year_label};
use crate::storage::ActivityEntry;

fn fmt_duration(secs: u64) -> String {
    if secs < 60 { return format!("{secs}s"); }
    let m = secs / 60;
    let h = m / 60;
    if h > 0 { format!("{h}h {m}m", m = m % 60) } else { format!("{m}m") }
}

fn fmt_time_ago(ts: i64) -> String {
    let diff = chrono::Utc::now().timestamp() - ts;
    if diff < 60 { "just now".into() }
    else if diff < 3600 { format!("{} min ago", diff / 60) }
    else if diff < 86400 { format!("{:.0}h ago", diff as f64 / 3600.0) }
    else { format!("{:.0}d ago", diff as f64 / 86400.0) }
}

fn action_label(action: &str) -> &str {
    match action {
        "downloaded" => "Downloaded",
        "streamed"   => "Streamed",
        "opened"     => "Opened",
        _            => "Accessed",
    }
}

fn render_course_card(
    ui: &mut egui::Ui,
    course: &Course,
    selected: &mut Option<u64>,
    metrics: &std::collections::HashMap<u64, (u64, u64)>,
    change_counts: &std::collections::HashMap<u64, u32>,
    event: &mut Option<CoursesEvent>,
) {
    let name = decode_html(&course.fullname);
    let is_selected = *selected == Some(course.id);
    let (_, total_secs) = metrics.get(&course.id).copied().unwrap_or((0, 0));
    let unseen = change_counts.get(&course.id).copied().unwrap_or(0);
    let bg = if is_selected { egui::Color32::from_rgb(40, 60, 90) }
             else { ui.visuals().faint_bg_color };

    ui.add_space(3.0);
    let avail_w = ui.available_width();
    let frame_resp = egui::Frame::none()
        .fill(bg)
        .rounding(5.0)
        .inner_margin(egui::Margin::symmetric(12.0, 10.0))
        .show(ui, |ui| {
            ui.set_min_width(avail_w - 24.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&name).size(15.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if unseen > 0 {
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgb(180, 80, 30))
                            .rounding(8.0)
                            .inner_margin(egui::Margin::symmetric(6.0, 1.0))
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new(format!("{unseen} new"))
                                    .size(10.0).strong()
                                    .color(egui::Color32::WHITE));
                            });
                    }
                    if total_secs > 0 {
                        ui.label(egui::RichText::new(fmt_duration(total_secs))
                            .size(11.0).color(egui::Color32::from_rgb(120, 180, 120)));
                    }
                });
            });
            ui.add_space(2.0);
            ui.label(egui::RichText::new(&course.shortname).size(11.0)
                .color(ui.visuals().weak_text_color()));
        });

    if frame_resp.response.interact(egui::Sense::click()).clicked() {
        if is_selected {
            *selected = None;
            *event = Some(CoursesEvent::Deselected);
        } else {
            *selected = Some(course.id);
            *event = Some(CoursesEvent::Selected(course.id));
        }
    }
    ui.add_space(3.0);
}

pub enum CoursesEvent {
    Selected(u64),
    Deselected,
}

pub struct CoursesScreen {
    pub courses: Vec<Course>,
    pub search: String,
    pub year_filter: Option<u8>,
    pub dept_filter: Option<String>,
    pub selected_course: Option<u64>,
    pub metrics: std::collections::HashMap<u64, (u64, u64)>,
    pub change_counts: std::collections::HashMap<u64, u32>,
    pub recent_activity: Vec<ActivityEntry>,
    pub show_recent: bool,
}

impl Default for CoursesScreen {
    fn default() -> Self {
        Self {
            courses: vec![],
            search: String::new(),
            year_filter: None,
            dept_filter: None,
            selected_course: None,
            metrics: Default::default(),
            change_counts: Default::default(),
            recent_activity: vec![],
            show_recent: true,
        }
    }
}

impl CoursesScreen {
    pub fn show(&mut self, ui: &mut Ui) -> Option<CoursesEvent> {
        let mut event: Option<CoursesEvent> = None;

        let mut years: BTreeSet<u8> = BTreeSet::new();
        let mut depts: BTreeSet<String> = BTreeSet::new();
        for c in &self.courses {
            if let Some(y) = parse_year(&c.shortname) { years.insert(y); }
            if let Some(d) = parse_dept(&c.shortname) { depts.insert(d); }
        }

        egui::TopBottomPanel::top("courses_topbar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("My Courses").size(16.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search)
                            .desired_width(200.0)
                            .hint_text("Search courses..."),
                    );
                });
            });
        });

        egui::SidePanel::left("filter_panel")
            .resizable(false)
            .default_width(160.0)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(6.0);

                    ui.label(egui::RichText::new("Year").size(13.0).strong());
                    ui.separator();
                    if ui.selectable_label(self.year_filter.is_none(), "All years").clicked() {
                        self.year_filter = None;
                    }
                    for &y in &years {
                        let label = year_label(y);
                        let sel = self.year_filter == Some(y);
                        if ui.selectable_label(sel, label).clicked() {
                            self.year_filter = if sel { None } else { Some(y) };
                        }
                    }

                    ui.add_space(10.0);

                    ui.label(egui::RichText::new("Department").size(13.0).strong());
                    ui.separator();
                    if ui.selectable_label(self.dept_filter.is_none(), "All depts").clicked() {
                        self.dept_filter = None;
                    }
                    for dept in &depts {
                        let sel = self.dept_filter.as_deref() == Some(dept.as_str());
                        if ui.selectable_label(sel, dept).clicked() {
                            self.dept_filter = if sel { None } else { Some(dept.clone()) };
                        }
                    }
                });
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if self.courses.is_empty() {
                ui.centered_and_justified(|ui| { ui.spinner(); });
                return;
            }

            let search_lower = self.search.to_lowercase();

            let visible: Vec<&Course> = self.courses.iter().filter(|c| {
                let year_ok = self.year_filter.map_or(true, |y| parse_year(&c.shortname) == Some(y));
                let dept_ok = self.dept_filter.as_ref().map_or(true, |d| parse_dept(&c.shortname).as_deref() == Some(d.as_str()));
                let search_ok = search_lower.is_empty()
                    || c.fullname.to_lowercase().contains(&search_lower)
                    || c.shortname.to_lowercase().contains(&search_lower);
                year_ok && dept_ok && search_ok
            }).collect();

            let mut group_map: std::collections::HashMap<u8, Vec<&Course>> = std::collections::HashMap::new();
            let mut other: Vec<&Course> = Vec::new();
            for c in &visible {
                match parse_year(&c.shortname) {
                    Some(y) => group_map.entry(y).or_default().push(c),
                    None    => other.push(c),
                }
            }
            let mut year_keys: Vec<u8> = group_map.keys().copied().collect();
            year_keys.sort_unstable_by(|a, b| b.cmp(a));

            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                // ── Recently Accessed ────────────────────────────────────────
                if !self.recent_activity.is_empty() {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let header = egui::RichText::new(
                            format!("{} Recently Accessed", egui_phosphor::regular::CLOCK_CLOCKWISE))
                            .size(13.0).strong();
                        ui.label(header);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let arrow = if self.show_recent { "▲" } else { "▼" };
                            if ui.small_button(arrow).clicked() {
                                self.show_recent = !self.show_recent;
                            }
                        });
                    });
                    if self.show_recent {
                        ui.add_space(2.0);
                        for entry in &self.recent_activity {
                            let course_name = self.courses.iter()
                                .find(|c| c.id == entry.course_id)
                                .map(|c| c.shortname.as_str())
                                .unwrap_or(&entry.course_name);
                            let resp = egui::Frame::none()
                                .fill(ui.visuals().faint_bg_color)
                                .rounding(4.0)
                                .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(egui_phosphor::regular::CLOCK_CLOCKWISE)
                                            .size(11.0).color(ui.visuals().weak_text_color()));
                                        ui.label(egui::RichText::new(&entry.module_name)
                                            .size(12.0).color(egui::Color32::from_rgb(100, 160, 230)));
                                        ui.label(egui::RichText::new(format!("— {course_name}"))
                                            .size(11.0).color(ui.visuals().weak_text_color()));
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            ui.label(egui::RichText::new(fmt_time_ago(entry.timestamp))
                                                .size(10.0).color(ui.visuals().weak_text_color()));
                                            ui.label(egui::RichText::new(action_label(&entry.action))
                                                .size(10.0).color(egui::Color32::from_rgb(120, 180, 120)));
                                        });
                                    });
                                });
                            if resp.response.interact(egui::Sense::click()).clicked() {
                                let cid = entry.course_id;
                                self.selected_course = Some(cid);
                                event = Some(CoursesEvent::Selected(cid));
                            }
                            if resp.response.interact(egui::Sense::hover()).hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            ui.add_space(2.0);
                        }
                        ui.add_space(4.0);
                        ui.separator();
                    }
                }

                // ── Course groups ────────────────────────────────────────────
                for year_num in &year_keys {
                    let group = year_label(*year_num);
                    let courses = &group_map[year_num];
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(group).size(15.0).strong());
                    ui.separator();
                    ui.add_space(2.0);

                    for course in courses {
                        render_course_card(ui, course, &mut self.selected_course,
                            &self.metrics, &self.change_counts, &mut event);
                    }
                }

                if !other.is_empty() {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Other").size(15.0).strong());
                    ui.separator();
                    ui.add_space(2.0);
                    for course in &other {
                        render_course_card(ui, course, &mut self.selected_course,
                            &self.metrics, &self.change_counts, &mut event);
                    }
                }

                if visible.is_empty() {
                    ui.add_space(20.0);
                    ui.centered_and_justified(|ui| {
                        ui.label(egui::RichText::new("No courses match filter")
                            .color(ui.visuals().weak_text_color()));
                    });
                }
            });
        });

        event
    }
}

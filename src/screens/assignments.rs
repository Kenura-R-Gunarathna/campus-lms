use egui::Ui;
use chrono::{DateTime, Local, Timelike, Utc};
use std::collections::BTreeSet;
use crate::api::types::{Assignment, AssignmentCourse, SubmissionStatusResponse};
use crate::models::{parse_dept, parse_year, year_label};

#[derive(PartialEq, Clone, Copy)]
pub enum StatusFilter { All, Upcoming, Overdue, NoDueDate, Completed }

impl StatusFilter {
    fn label(self) -> &'static str {
        match self {
            Self::All       => "All",
            Self::Upcoming  => "Upcoming",
            Self::Overdue   => "Overdue",
            Self::NoDueDate => "No Deadline",
            Self::Completed => "Completed",
        }
    }
}

pub enum AssignmentsEvent {
    RequestStatus(u64),
}

#[derive(Clone, Copy, PartialEq)]
enum Urgency { Overdue, Danger, Alert, Warning, Info, Scheduled }

impl Urgency {
    fn from_diff(diff: i64) -> Self {
        if diff < 0               { Self::Overdue }
        else if diff < 86_400     { Self::Danger  }  // < 24 h
        else if diff < 3 * 86_400 { Self::Alert   }  // < 3 d
        else if diff < 7 * 86_400 { Self::Warning }  // < 7 d
        else if diff < 14* 86_400 { Self::Info    }  // < 14 d
        else                      { Self::Scheduled }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Overdue   => "OVERDUE",
            Self::Danger    => "DANGER",
            Self::Alert     => "ALERT",
            Self::Warning   => "WARNING",
            Self::Info      => "INFO",
            Self::Scheduled => "",
        }
    }

    fn badge_color(self) -> egui::Color32 {
        match self {
            Self::Overdue   => egui::Color32::from_rgb(180, 40,  40),
            Self::Danger    => egui::Color32::from_rgb(210, 60,  60),
            Self::Alert     => egui::Color32::from_rgb(200, 120, 40),
            Self::Warning   => egui::Color32::from_rgb(180, 160, 30),
            Self::Info      => egui::Color32::from_rgb(50,  120, 200),
            Self::Scheduled => egui::Color32::TRANSPARENT,
        }
    }

    fn text_color(self) -> egui::Color32 {
        match self {
            Self::Warning => egui::Color32::from_rgb(30, 25, 0),
            _             => egui::Color32::WHITE,
        }
    }

    fn due_text_color(self) -> egui::Color32 {
        match self {
            Self::Overdue   => egui::Color32::from_rgb(220, 80,  80),
            Self::Danger    => egui::Color32::from_rgb(230, 100, 80),
            Self::Alert     => egui::Color32::from_rgb(220, 150, 50),
            Self::Warning   => egui::Color32::from_rgb(200, 180, 60),
            Self::Info      => egui::Color32::from_rgb(80,  160, 220),
            Self::Scheduled => egui::Color32::from_rgb(100, 180, 100),
        }
    }

    fn card_tint(self) -> egui::Color32 {
        match self {
            Self::Overdue => egui::Color32::from_rgba_premultiplied(80, 10, 10, 40),
            Self::Danger  => egui::Color32::from_rgba_premultiplied(70, 15, 15, 25),
            _             => egui::Color32::TRANSPARENT,
        }
    }
}

pub struct AssignmentsScreen {
    pub courses: Vec<AssignmentCourse>,
    pub status: StatusFilter,
    pub year_filter: Option<u8>,
    pub dept_filter: Option<String>,
    pub search: String,
    pub my_courses_only: bool,
    pub student_year: Option<u8>,
    pub submission_statuses: std::collections::HashMap<u64, SubmissionStatusResponse>,
    pub loading_statuses: std::collections::HashSet<u64>,
}

impl Default for AssignmentsScreen {
    fn default() -> Self {
        Self {
            courses: vec![],
            status: StatusFilter::Upcoming,
            year_filter: None,
            dept_filter: None,
            search: String::new(),
            my_courses_only: true,
            student_year: None,
            submission_statuses: Default::default(),
            loading_statuses: Default::default(),
        }
    }
}

fn fmt_due(ts: i64) -> (String, Option<&'static str>) {
    let dt: DateTime<Local> = DateTime::from(DateTime::<Utc>::from_timestamp(ts, 0).unwrap());
    // Include day-of-week so Mon/Tue context is visible alongside the date
    let base = dt.format("%a %d %b %Y, %H:%M").to_string();
    // 00:00 is the START of a day, not the end — make that unambiguous
    let midnight_note = if dt.hour() == 0 && dt.minute() == 0 {
        Some("midnight = start of this day, not end of previous")
    } else if dt.hour() == 23 && dt.minute() == 59 {
        Some("11:59 PM = end of this day")
    } else {
        None
    };
    (base, midnight_note)
}

fn fmt_remaining(diff: i64) -> String {
    fmt_duration(diff, "left")
}

fn fmt_overdue(diff: i64) -> String {
    fmt_duration(diff, "overdue")
}

fn fmt_duration(secs: i64, suffix: &str) -> String {
    if secs < 3600 {
        format!("{} min {suffix}", secs / 60)
    } else if secs < 86_400 {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        if m > 0 { format!("{h}h {m}m {suffix}") } else { format!("{h}h {suffix}") }
    } else {
        let d = secs / 86_400;
        let h = (secs % 86_400) / 3600;
        if h > 0 { format!("{d}d {h}h {suffix}") } else { format!("{d}d {suffix}") }
    }
}

impl AssignmentsScreen {
    pub fn show(&mut self, ui: &mut Ui) -> Option<AssignmentsEvent> {
        let mut event = None;
        let now = Utc::now().timestamp();

        let mut years: BTreeSet<u8> = BTreeSet::new();
        let mut depts: BTreeSet<String> = BTreeSet::new();
        for c in &self.courses {
            if let Some(y) = parse_year(&c.shortname) { years.insert(y); }
            if let Some(d) = parse_dept(&c.shortname) { depts.insert(d); }
        }

        egui::TopBottomPanel::top("assign_topbar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Assignments").size(16.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.search)
                        .desired_width(180.0).hint_text("Search..."));
                });
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                for &s in &[StatusFilter::Upcoming, StatusFilter::Overdue, StatusFilter::Completed, StatusFilter::NoDueDate, StatusFilter::All] {
                    if ui.selectable_label(self.status == s, s.label()).clicked() {
                        self.status = s;
                    }
                }
                ui.separator();
                ui.checkbox(&mut self.my_courses_only, "My courses only");
            });
        });

        egui::SidePanel::left("assign_filter")
            .resizable(false)
            .default_width(155.0)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Year").size(13.0).strong());
                    ui.separator();
                    if ui.selectable_label(self.year_filter.is_none(), "All years").clicked() {
                        self.year_filter = None;
                    }
                    for &y in &years {
                        let sel = self.year_filter == Some(y);
                        if ui.selectable_label(sel, year_label(y)).clicked() {
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

            let mut all: Vec<(&AssignmentCourse, &Assignment)> = self.courses.iter()
                .filter(|c| {
                    let year_ok = self.year_filter.map_or(true, |y| parse_year(&c.shortname) == Some(y));
                    let dept_ok = self.dept_filter.as_ref().map_or(true, |d| parse_dept(&c.shortname).as_deref() == Some(d.as_str()));
                    
                    let relevance_ok = if self.my_courses_only {
                         self.student_year.map_or(true, |y| parse_year(&c.shortname) == Some(y))
                    } else {
                        true
                    };

                    year_ok && dept_ok && relevance_ok
                })
                .flat_map(|c| c.assignments.iter().map(move |a| (c, a)))
                .filter(|(c, a)| {
                    let is_submitted = self.submission_statuses.get(&a.id)
                        .and_then(|s| s.lastattempt.as_ref())
                        .and_then(|l| l.submission.as_ref())
                        .map_or(false, |s| s.status == "submitted");

                    let status_ok = match self.status {
                        StatusFilter::All       => true,
                        StatusFilter::Upcoming  => a.duedate > now && !is_submitted,
                        StatusFilter::Overdue   => a.duedate > 0 && a.duedate <= now && !is_submitted,
                        StatusFilter::NoDueDate => a.duedate == 0 && !is_submitted,
                        StatusFilter::Completed => is_submitted,
                    };
                    let search_ok = search_lower.is_empty()
                        || a.name.to_lowercase().contains(&search_lower)
                        || c.shortname.to_lowercase().contains(&search_lower);
                    status_ok && search_ok
                })
                .collect();

            all.sort_by(|(_, a), (_, b)| {
                if a.duedate == b.duedate { return std::cmp::Ordering::Equal; }
                if a.duedate == 0 { return std::cmp::Ordering::Greater; }
                if b.duedate == 0 { return std::cmp::Ordering::Less; }
                
                if self.status == StatusFilter::Overdue {
                    b.duedate.cmp(&a.duedate) // descending
                } else {
                    a.duedate.cmp(&b.duedate) // ascending
                }
            });

            if all.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(30.0);
                    ui.label(egui::RichText::new("No assignments match filter")
                        .color(ui.visuals().weak_text_color()));
                });
                return;
            }

            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for (course, assign) in all {
                    let urgency = if assign.duedate > 0 {
                        Urgency::from_diff(assign.duedate - now)
                    } else {
                        Urgency::Scheduled
                    };

                    ui.add_space(4.0);

                    let base_fill = ui.visuals().faint_bg_color;
                    let tint = urgency.card_tint();
                    let fill = if tint != egui::Color32::TRANSPARENT {
                        // blend tint over base
                        let [br, bg, bb, _] = base_fill.to_array();
                        let [tr, tg, tb, ta] = tint.to_array();
                        let a = ta as f32 / 255.0;
                        egui::Color32::from_rgb(
                            (br as f32 * (1.0 - a) + tr as f32 * a) as u8,
                            (bg as f32 * (1.0 - a) + tg as f32 * a) as u8,
                            (bb as f32 * (1.0 - a) + tb as f32 * a) as u8,
                        )
                    } else {
                        base_fill
                    };

                    egui::Frame::none()
                        .fill(fill)
                        .rounding(5.0)
                        .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Urgency badge
                                let badge_label = urgency.label();
                                if !badge_label.is_empty() {
                                    egui::Frame::none()
                                        .fill(urgency.badge_color())
                                        .rounding(4.0)
                                        .inner_margin(egui::Margin::symmetric(6.0, 2.0))
                                        .show(ui, |ui| {
                                            ui.label(egui::RichText::new(badge_label)
                                                .size(10.0).strong().color(urgency.text_color()));
                                        });
                                }

                                // Submitted badge
                                let status = self.submission_statuses.get(&assign.id);
                                let is_submitted = status
                                    .and_then(|s| s.lastattempt.as_ref())
                                    .and_then(|l| l.submission.as_ref())
                                    .map_or(false, |s| s.status == "submitted");

                                if is_submitted {
                                    egui::Frame::none()
                                        .fill(egui::Color32::from_rgb(40, 120, 40))
                                        .rounding(4.0)
                                        .inner_margin(egui::Margin::symmetric(6.0, 2.0))
                                        .show(ui, |ui| {
                                            ui.label(egui::RichText::new("SUBMITTED")
                                                .size(10.0).strong().color(egui::Color32::WHITE));
                                        });
                                }

                                ui.label(egui::RichText::new(&assign.name).size(15.0));
                                
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if !is_submitted && status.is_none() {
                                        if self.loading_statuses.contains(&assign.id) {
                                            ui.add(egui::Spinner::new().size(12.0));
                                        } else if ui.small_button("Check Status").clicked() {
                                            event = Some(AssignmentsEvent::RequestStatus(assign.id));
                                        }
                                    }
                                    ui.label(egui::RichText::new(&course.shortname).size(11.0)
                                        .color(ui.visuals().weak_text_color()));
                                });
                            });
                            ui.add_space(4.0);
                            if assign.duedate > 0 {
                                let diff = assign.duedate - now;
                                let color = urgency.due_text_color();
                                let (due_str, midnight_note) = fmt_due(assign.duedate);
                                let label = if diff < 0 {
                                    format!("Overdue — was due {} ({})", due_str, fmt_overdue(diff.abs()))
                                } else {
                                    format!("Due: {} — {}", due_str, fmt_remaining(diff))
                                };
                                ui.label(egui::RichText::new(label).size(12.0).color(color));
                                if let Some(note) = midnight_note {
                                    ui.label(egui::RichText::new(format!("! {note}"))
                                        .size(11.0).color(egui::Color32::from_rgb(255, 200, 80)));
                                }
                            } else {
                                ui.label(egui::RichText::new("No deadline")
                                    .size(12.0).color(ui.visuals().weak_text_color()));
                            }
                        });
                }
            });
        });

        event
    }
}

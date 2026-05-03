use egui::Ui;
use std::collections::{BTreeSet, HashMap, HashSet};
use crate::api::types::{GradeItem, UserGrades};
use crate::models::{parse_dept, parse_year, year_label};

pub struct GradesScreen {
    pub grades: Vec<UserGrades>,
    pub overview: HashMap<u64, String>,          // courseid → overview grade (fast)
    pub course_list: Vec<(u64, String, String)>, // (id, fullname, shortname)
    pub detail_loading: HashSet<u64>,
    pub year_filter: Option<u8>,
    pub dept_filter: Option<String>,
    pub expanded: HashSet<u64>,
    pub search: String,
}

impl Default for GradesScreen {
    fn default() -> Self {
        Self {
            grades: vec![],
            overview: HashMap::new(),
            course_list: vec![],
            detail_loading: HashSet::new(),
            year_filter: None,
            dept_filter: None,
            expanded: HashSet::new(),
            search: String::new(),
        }
    }
}

fn grade_color(formatted: &str) -> egui::Color32 {
    let num: Option<f64> = formatted
        .split_whitespace().next()
        .and_then(|s| s.trim_end_matches('%').parse().ok());
    match num {
        Some(v) if v >= 75.0 => egui::Color32::from_rgb(80, 180, 80),
        Some(v) if v >= 50.0 => egui::Color32::from_rgb(200, 180, 60),
        Some(v) if v >= 0.0  => egui::Color32::from_rgb(200, 80, 80),
        _ => egui::Color32::GRAY,
    }
}


impl GradesScreen {
    /// Returns a course_id that needs its detail grades loaded, if any.
    pub fn show(&mut self, ui: &mut Ui) -> Option<u64> {
        let mut needs_load: Option<u64> = None;

        let mut years: BTreeSet<u8> = BTreeSet::new();
        let mut depts: BTreeSet<String> = BTreeSet::new();
        for (_, _, sn) in &self.course_list {
            if let Some(y) = parse_year(sn) { years.insert(y); }
            if let Some(d) = parse_dept(sn) { depts.insert(d); }
        }

        egui::TopBottomPanel::top("grades_topbar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Grades").size(16.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.search)
                        .desired_width(180.0).hint_text("Search..."));
                });
            });
        });

        egui::SidePanel::left("grades_filter")
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
            if self.course_list.is_empty() && self.overview.is_empty() {
                ui.centered_and_justified(|ui| { ui.spinner(); });
                return;
            }

            let search_lower = self.search.to_lowercase();

            let visible: Vec<(u64, &str, &str)> = self.course_list.iter()
                .filter(|(_, fullname, sn)| {
                    let year_ok = self.year_filter.map_or(true, |y| parse_year(sn) == Some(y));
                    let dept_ok = self.dept_filter.as_ref().map_or(true, |d| parse_dept(sn).as_deref() == Some(d.as_str()));
                    let search_ok = search_lower.is_empty()
                        || fullname.to_lowercase().contains(&search_lower);
                    year_ok && dept_ok && search_ok
                })
                .map(|(id, fullname, sn)| (*id, fullname.as_str(), sn.as_str()))
                .collect();

            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for (course_id, fullname, _sn) in visible {
                    let is_expanded = self.expanded.contains(&course_id);
                    let detail = self.grades.iter().find(|g| g.courseid == course_id);
                    let is_loading = self.detail_loading.contains(&course_id);

                    ui.add_space(4.0);
                    egui::Frame::none()
                        .fill(ui.visuals().faint_bg_color)
                        .rounding(5.0)
                        .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                        .show(ui, |ui| {
                            let resp = ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(if is_expanded { egui_phosphor::regular::CARET_DOWN } else { egui_phosphor::regular::CARET_RIGHT })
                                    .size(11.0).color(ui.visuals().weak_text_color()));
                                ui.label(egui::RichText::new(fullname).size(15.0));

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    // Show detail total if loaded, otherwise overview grade
                                    if let Some(ug) = detail {
                                        if let Some(total) = ug.gradeitems.iter().rev().find(|i| {
                                            i.itemname.as_deref().map_or(false, |n| n.to_lowercase().contains("total"))
                                                || i.itemname.is_none()
                                        }) {
                                            let color = grade_color(&total.gradeformatted);
                                            ui.label(egui::RichText::new(&total.gradeformatted)
                                                .size(14.0).strong().color(color));
                                        }
                                    } else if let Some(ov) = self.overview.get(&course_id) {
                                        let color = grade_color(ov);
                                        ui.label(egui::RichText::new(ov).size(14.0).strong().color(color));
                                    }
                                });
                            });

                            if resp.response.interact(egui::Sense::click()).clicked() {
                                if is_expanded {
                                    self.expanded.remove(&course_id);
                                } else {
                                    self.expanded.insert(course_id);
                                    // Signal detail load needed
                                    if detail.is_none() && !is_loading {
                                        needs_load = Some(course_id);
                                    }
                                }
                            }

                            if is_expanded {
                                ui.separator();
                                if is_loading {
                                    ui.horizontal(|ui| {
                                        ui.spinner();
                                        ui.label(egui::RichText::new("Loading grades…")
                                            .size(12.0).color(ui.visuals().weak_text_color()));
                                    });
                                } else if let Some(ug) = detail {
                                    let items: Vec<&GradeItem> = ug.gradeitems.iter()
                                        .filter(|i| {
                                            let is_total = i.itemname.as_deref()
                                                .map_or(false, |n| n.to_lowercase().contains("total"));
                                            !is_total && !i.gradeformatted.is_empty() && i.gradeformatted != "-"
                                        })
                                        .collect();

                                    if items.is_empty() {
                                        ui.label(egui::RichText::new("No graded items yet")
                                            .size(12.0).color(ui.visuals().weak_text_color()));
                                    }

                                    for item in items {
                                        ui.add_space(3.0);
                                        ui.horizontal(|ui| {
                                            let name = item.itemname.as_deref().unwrap_or("Unnamed");
                                            ui.label(egui::RichText::new(name).size(13.0));
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if let Some(pct) = &item.percentageformatted {
                                                    let color = grade_color(pct);
                                                    ui.label(egui::RichText::new(pct).size(12.0).color(color));
                                                    ui.label(egui::RichText::new(" | ").size(12.0)
                                                        .color(ui.visuals().weak_text_color()));
                                                }
                                                ui.label(egui::RichText::new(&item.gradeformatted)
                                                    .size(12.0).strong());
                                            });
                                        });
                                        if let Some(fb) = &item.feedback {
                                            if !fb.is_empty() {
                                                ui.label(egui::RichText::new(fb).size(11.0).italics()
                                                    .color(ui.visuals().weak_text_color()));
                                            }
                                        }
                                    }
                                }
                            }
                        });
                }
            });
        });

        needs_load
    }
}

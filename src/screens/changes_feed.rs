use egui::Ui;
use std::collections::HashMap;
use similar::{ChangeTag, TextDiff};
use crate::storage::ContentChange;

pub struct ChangesFeedScreen {
    pub changes: Vec<ContentChange>,
    pub course_names: HashMap<u64, String>,
    pub selected_id: Option<i64>,
    pub filter_course: Option<u64>,
}

impl Default for ChangesFeedScreen {
    fn default() -> Self {
        Self {
            changes: vec![],
            course_names: HashMap::new(),
            selected_id: None,
            filter_course: None,
        }
    }
}

impl ChangesFeedScreen {
    pub fn load(&mut self, changes: Vec<ContentChange>, course_names: HashMap<u64, String>) {
        self.changes = changes;
        self.course_names = course_names;
        self.selected_id = self.changes.first().map(|c| c.id);
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let mut new_selected = self.selected_id;
        let mut new_filter = self.filter_course;

        egui::SidePanel::left("cf_left")
            .resizable(false)
            .exact_width(280.0)
            .show_inside(ui, |ui| {
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Changes").size(14.0).strong());
                ui.add_space(4.0);

                // Course filter row
                ui.horizontal_wrapped(|ui| {
                    let all_active = self.filter_course.is_none();
                    if ui.selectable_label(all_active, "All courses").clicked() {
                        new_filter = None;
                    }
                    // Collect unique course ids from changes
                    let mut seen: std::collections::HashSet<u64> = Default::default();
                    for ch in &self.changes {
                        if seen.insert(ch.course_id) {
                            let name = self.course_names.get(&ch.course_id)
                                .map(|n| truncate(n, 20))
                                .unwrap_or_else(|| format!("#{}", ch.course_id));
                            let active = self.filter_course == Some(ch.course_id);
                            if ui.selectable_label(active, name).clicked() {
                                new_filter = Some(ch.course_id);
                            }
                        }
                    }
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);

                let filtered: Vec<&ContentChange> = self.changes.iter()
                    .filter(|c| self.filter_course.map_or(true, |id| c.course_id == id))
                    .collect();

                if filtered.is_empty() {
                    ui.add_space(20.0);
                    ui.centered_and_justified(|ui| {
                        ui.label(egui::RichText::new("No changes recorded yet.\nOpen courses to start tracking.")
                            .color(ui.visuals().weak_text_color()));
                    });
                    return;
                }

                egui::ScrollArea::vertical()
                    .id_salt("cf_list_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for ch in &filtered {
                            let is_selected = Some(ch.id) == self.selected_id;
                            let (icon, color) = change_icon_color(&ch.change_type);

                            let fill = if is_selected {
                                egui::Color32::from_rgba_premultiplied(60, 50, 20, 80)
                            } else {
                                egui::Color32::TRANSPARENT
                            };

                            let card = egui::Frame::none()
                                .fill(fill)
                                .rounding(4.0)
                                .inner_margin(egui::Margin::symmetric(6.0, 4.0))
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                                        ui.label(egui::RichText::new(icon).color(color).size(13.0));
                                        ui.label(egui::RichText::new(truncate(&ch.module_name, 26))
                                            .size(12.0));
                                    });
                                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                                        let course_label = self.course_names.get(&ch.course_id)
                                            .map(|n| truncate(n, 22))
                                            .unwrap_or_else(|| format!("Course #{}", ch.course_id));
                                        ui.label(egui::RichText::new(course_label)
                                            .size(10.0)
                                            .color(egui::Color32::from_rgb(150, 180, 255)));
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                                            ui.label(egui::RichText::new(fmt_time_ago(ch.detected_at))
                                                .size(9.5)
                                                .color(ui.visuals().weak_text_color()));
                                        });
                                    });
                                });

                            let resp = ui.interact(
                                card.response.rect,
                                egui::Id::new(("cf_entry", ch.id)),
                                egui::Sense::click(),
                            );
                            if resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if resp.clicked() {
                                new_selected = Some(ch.id);
                            }
                            ui.add_space(1.0);
                        }
                    });
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            let selected = self.selected_id.and_then(|id| {
                self.changes.iter().find(|c| c.id == id)
            });

            match selected {
                None => {
                    ui.add_space(80.0);
                    ui.centered_and_justified(|ui| {
                        ui.label(egui::RichText::new("Select a change on the left to see the diff.")
                            .color(ui.visuals().weak_text_color()));
                    });
                }
                Some(ch) => {
                    render_detail(ui, ch, &self.course_names);
                }
            }
        });

        self.selected_id = new_selected;
        self.filter_course = new_filter;
    }
}

fn render_detail(ui: &mut Ui, ch: &ContentChange, course_names: &HashMap<u64, String>) {
    let (icon, color) = change_icon_color(&ch.change_type);
    let course_name = course_names.get(&ch.course_id)
        .cloned()
        .unwrap_or_else(|| format!("Course #{}", ch.course_id));

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(icon).color(color).size(16.0));
        ui.label(egui::RichText::new(&ch.module_name).size(14.0).strong());
    });
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&course_name)
            .size(11.0).color(egui::Color32::from_rgb(150, 180, 255)));
        if !ch.section_name.is_empty() {
            ui.label(egui::RichText::new(format!("/ {}", ch.section_name))
                .size(11.0).color(ui.visuals().weak_text_color()));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(fmt_time_ago(ch.detected_at))
                .size(10.0).color(ui.visuals().weak_text_color()));
        });
    });
    ui.separator();
    ui.add_space(6.0);

    match ch.change_type.as_str() {
        "description_updated" => {
            render_side_by_side(ui, &ch.old_val, &ch.new_val);
        }
        "content_updated" => {
            egui::Frame::none()
                .fill(ui.visuals().faint_bg_color)
                .rounding(4.0)
                .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Module content, files, or dates were updated.")
                        .size(13.0).color(egui::Color32::from_rgb(255, 200, 100)));
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("The system detected a change in the files inside this module, its description, or its associated dates (e.g. assignment due dates).")
                        .size(11.0).color(ui.visuals().weak_text_color()));
                });
        }
        "section_updated" => {
            egui::Frame::none()
                .fill(ui.visuals().faint_bg_color)
                .rounding(4.0)
                .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Section summary or name updated.")
                        .size(13.0).color(egui::Color32::from_rgb(180, 160, 255)));
                });
        }
        "renamed" => {
            egui::Frame::none()
                .fill(ui.visuals().faint_bg_color)
                .rounding(4.0)
                .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Renamed").size(11.0)
                        .color(ui.visuals().weak_text_color()));
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(&ch.old_val).size(13.0)
                        .color(egui::Color32::from_rgb(220, 100, 100)));
                    ui.label(egui::RichText::new("→").size(11.0)
                        .color(ui.visuals().weak_text_color()));
                    ui.label(egui::RichText::new(&ch.new_val).size(13.0)
                        .color(egui::Color32::from_rgb(100, 220, 100)));
                });
        }
        "file_updated" => {
            egui::Frame::none()
                .fill(ui.visuals().faint_bg_color)
                .rounding(4.0)
                .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("File updated").size(11.0)
                        .color(ui.visuals().weak_text_color()));
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&ch.old_val).size(13.0)
                            .color(egui::Color32::from_rgb(220, 100, 100)));
                        ui.label(egui::RichText::new("→").size(11.0)
                            .color(ui.visuals().weak_text_color()));
                        ui.label(egui::RichText::new(&ch.new_val).size(13.0)
                            .color(egui::Color32::from_rgb(100, 220, 100)));
                    });
                });
        }
        "added" => {
            egui::Frame::none()
                .fill(egui::Color32::from_rgba_premultiplied(0, 60, 0, 60))
                .rounding(4.0)
                .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(
                        format!("New content added in \"{}\"", ch.section_name))
                        .size(13.0).color(egui::Color32::from_rgb(100, 220, 100)));
                });
        }
        "removed" => {
            egui::Frame::none()
                .fill(egui::Color32::from_rgba_premultiplied(60, 0, 0, 60))
                .rounding(4.0)
                .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(
                        format!("Content removed from \"{}\"", ch.section_name))
                        .size(13.0).color(egui::Color32::from_rgb(220, 100, 100)));
                });
        }
        _ => {}
    }
}

fn render_side_by_side(ui: &mut Ui, old: &str, new: &str) {
    let diff = TextDiff::from_lines(old, new);

    // Build two aligned line vecs: (background_color, text)
    let transparent = egui::Color32::TRANSPARENT;
    let del_bg = egui::Color32::from_rgba_premultiplied(80, 0, 0, 100);
    let ins_bg = egui::Color32::from_rgba_premultiplied(0, 80, 0, 100);

    let mut left: Vec<(egui::Color32, String)> = vec![];
    let mut right: Vec<(egui::Color32, String)> = vec![];

    for op in diff.ops() {
        let dels: Vec<String> = diff.iter_changes(op)
            .filter(|c| c.tag() == ChangeTag::Delete)
            .map(|c| c.value().trim_end_matches('\n').to_string())
            .collect();
        let ins: Vec<String> = diff.iter_changes(op)
            .filter(|c| c.tag() == ChangeTag::Insert)
            .map(|c| c.value().trim_end_matches('\n').to_string())
            .collect();
        let eqs: Vec<String> = diff.iter_changes(op)
            .filter(|c| c.tag() == ChangeTag::Equal)
            .map(|c| c.value().trim_end_matches('\n').to_string())
            .collect();

        for line in &eqs {
            left.push((transparent, line.clone()));
            right.push((transparent, line.clone()));
        }

        let max = dels.len().max(ins.len());
        for i in 0..max {
            left.push((del_bg, dels.get(i).cloned().unwrap_or_default()));
            right.push((ins_bg, ins.get(i).cloned().unwrap_or_default()));
        }
    }

    egui::ScrollArea::vertical()
        .id_salt("cf_sidebyside_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.columns(2, |cols| {
                cols[0].label(egui::RichText::new("Before")
                    .size(11.0).strong()
                    .color(egui::Color32::from_rgb(220, 100, 100)));
                cols[0].separator();
                for (bg, line) in &left {
                    let rt = egui::RichText::new(if line.is_empty() { " " } else { line.as_str() })
                        .size(11.0).monospace();
                    let rt = if *bg != transparent { rt.background_color(*bg) } else { rt };
                    cols[0].label(rt);
                }

                cols[1].label(egui::RichText::new("After")
                    .size(11.0).strong()
                    .color(egui::Color32::from_rgb(100, 220, 100)));
                cols[1].separator();
                for (bg, line) in &right {
                    let rt = egui::RichText::new(if line.is_empty() { " " } else { line.as_str() })
                        .size(11.0).monospace();
                    let rt = if *bg != transparent { rt.background_color(*bg) } else { rt };
                    cols[1].label(rt);
                }
            });
        });
}

fn change_icon_color(change_type: &str) -> (&'static str, egui::Color32) {
    match change_type {
        "added"               => (egui_phosphor::regular::PLUS_CIRCLE, egui::Color32::from_rgb(80, 200, 80)),
        "removed"             => (egui_phosphor::regular::MINUS_CIRCLE, egui::Color32::from_rgb(200, 80, 80)),
        "renamed"             => (egui_phosphor::regular::PENCIL_SIMPLE, egui::Color32::from_rgb(150, 180, 255)),
        "file_updated"        => (egui_phosphor::regular::ARROW_CLOCKWISE, egui::Color32::from_rgb(255, 180, 60)),
        "description_updated" => (egui_phosphor::regular::TEXT_T, egui::Color32::from_rgb(200, 160, 255)),
        "content_updated"     => (egui_phosphor::regular::FILE_ARROW_UP, egui::Color32::from_rgb(255, 180, 60)),
        "section_updated"     => (egui_phosphor::regular::FOLDER_NOTCH_OPEN, egui::Color32::from_rgb(180, 160, 255)),
        _                     => (egui_phosphor::regular::DOT, egui::Color32::GRAY),
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let end = s.char_indices().nth(max_chars).map(|(i, _)| i).unwrap_or(s.len());
        format!("{}…", &s[..end])
    }
}

fn fmt_time_ago(ts: i64) -> String {
    let diff = chrono::Utc::now().timestamp() - ts;
    if diff < 60 { "just now".into() }
    else if diff < 3600 { format!("{} min ago", diff / 60) }
    else if diff < 86400 { format!("{:.0}h ago", diff as f64 / 3600.0) }
    else { format!("{:.0}d ago", diff as f64 / 86400.0) }
}

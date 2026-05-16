use egui::Ui;
use crate::storage::ContentChange;

pub enum DiffHistoryEvent {
    Back,
}

pub struct SnapshotGroup {
    pub hash: String,
    pub detected_at: i64,
    pub changes: Vec<ContentChange>,
}

pub struct DiffHistoryScreen {
    pub _course_id: u64,
    pub course_name: String,
    pub groups: Vec<SnapshotGroup>,
    pub selected_group: Option<usize>,
    pub compare_a: Option<usize>,
    pub compare_b: Option<usize>,
    pub expanded_changes: std::collections::HashSet<i64>,
    pub compare_mode: bool,
}

impl Default for DiffHistoryScreen {
    fn default() -> Self {
        Self {
            _course_id: 0,
            course_name: String::new(),
            groups: vec![],
            selected_group: None,
            compare_a: None,
            compare_b: None,
            expanded_changes: Default::default(),
            compare_mode: false,
        }
    }
}

fn snapshot_hash(course_id: u64, ts: i64) -> String {
    let mut h: u64 = 14695981039346656037;
    for b in course_id.to_le_bytes().iter().chain(ts.to_le_bytes().iter()) {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    let s = format!("{:016x}", h);
    s[..8].to_string()
}

pub fn group_into_snapshots(changes: Vec<ContentChange>) -> Vec<SnapshotGroup> {
    const SESSION_GAP_SECS: i64 = 300;
    let mut groups: Vec<SnapshotGroup> = vec![];
    for change in changes {
        let fits_last = groups.last()
            .map(|g: &SnapshotGroup| change.detected_at - g.detected_at < SESSION_GAP_SECS)
            .unwrap_or(false);
        if fits_last {
            groups.last_mut().unwrap().changes.push(change);
        } else {
            let hash = snapshot_hash(change.course_id, change.detected_at);
            let ts = change.detected_at;
            groups.push(SnapshotGroup { hash, detected_at: ts, changes: vec![change] });
        }
    }
    groups
}

fn fmt_ts(ts: i64) -> String {
    use chrono::{DateTime, Utc};
    DateTime::<Utc>::from_timestamp(ts, 0)
        .map(|dt| dt.format("%b %d  %H:%M").to_string())
        .unwrap_or_else(|| ts.to_string())
}

fn fmt_date(ts: i64) -> String {
    use chrono::{DateTime, Utc};
    DateTime::<Utc>::from_timestamp(ts, 0)
        .map(|dt| dt.format("%b %d").to_string())
        .unwrap_or_else(|| ts.to_string())
}

fn change_color(change_type: &str) -> egui::Color32 {
    match change_type {
        "added"               => egui::Color32::from_rgb(80, 200, 80),
        "removed"             => egui::Color32::from_rgb(200, 80, 80),
        "renamed"             => egui::Color32::from_rgb(150, 180, 255),
        "file_updated"        => egui::Color32::from_rgb(255, 180, 60),
        "description_updated" => egui::Color32::from_rgb(200, 160, 255),
        _                     => egui::Color32::GRAY,
    }
}

fn change_icon(change_type: &str) -> &'static str {
    match change_type {
        "added"               => egui_phosphor::regular::PLUS_CIRCLE,
        "removed"             => egui_phosphor::regular::MINUS_CIRCLE,
        "renamed"             => egui_phosphor::regular::PENCIL_SIMPLE,
        "file_updated"        => egui_phosphor::regular::ARROW_CLOCKWISE,
        "description_updated" => egui_phosphor::regular::TEXT_T,
        _                     => egui_phosphor::regular::DOT,
    }
}

fn render_diff_block(ui: &mut Ui, old: &str, new: &str) {
    let diff_text = crate::telemetry::text_diff(old, new);
    egui::Frame::none()
        .fill(egui::Color32::from_rgba_premultiplied(0, 0, 0, 60))
        .rounding(3.0)
        .inner_margin(egui::Margin::symmetric(6.0, 3.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            if diff_text.is_empty() {
                ui.label(egui::RichText::new("(no text changes)")
                    .size(10.0).color(ui.visuals().weak_text_color()));
                return;
            }
            for line in diff_text.lines() {
                if line.starts_with("+ ") {
                    ui.label(egui::RichText::new(line)
                        .size(10.5).monospace()
                        .background_color(egui::Color32::from_rgba_premultiplied(0, 80, 0, 100))
                        .color(egui::Color32::from_rgb(120, 220, 120)));
                } else if line.starts_with("- ") {
                    ui.label(egui::RichText::new(line)
                        .size(10.5).monospace()
                        .background_color(egui::Color32::from_rgba_premultiplied(80, 0, 0, 100))
                        .color(egui::Color32::from_rgb(220, 100, 100)));
                }
            }
        });
}

fn render_change_entry(
    ui: &mut Ui,
    ch: &ContentChange,
    is_expanded: bool,
) -> (bool, bool) {
    let color = change_color(&ch.change_type);
    let icon  = change_icon(&ch.change_type);

    let frame_resp = egui::Frame::none()
        .fill(if is_expanded {
            egui::Color32::from_rgba_premultiplied(30, 30, 50, 60)
        } else {
            egui::Color32::TRANSPARENT
        })
        .rounding(4.0)
        .inner_margin(egui::Margin::symmetric(4.0, 2.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(icon).color(color).size(12.0));
                ui.label(egui::RichText::new(&ch.module_name).size(12.0));
                if !ch.section_name.is_empty() {
                    ui.label(egui::RichText::new(format!("— {}", ch.section_name))
                        .size(11.0).color(ui.visuals().weak_text_color()));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(if is_expanded { egui_phosphor::regular::CARET_UP } else { egui_phosphor::regular::CARET_DOWN })
                        .size(9.0).color(ui.visuals().weak_text_color()));
                    ui.label(egui::RichText::new(&ch.change_type)
                        .size(10.0).color(color));
                });
            });

            if is_expanded {
                ui.add_space(3.0);
                match ch.change_type.as_str() {
                    "description_updated" => {
                        render_diff_block(ui, &ch.old_val, &ch.new_val);
                    }
                    "renamed" => {
                        ui.label(egui::RichText::new(format!("\"{}\" → \"{}\"", ch.old_val, ch.new_val))
                            .size(11.0).color(color));
                    }
                    "file_updated" => {
                        ui.label(egui::RichText::new(format!("Size: {} → {}", ch.old_val, ch.new_val))
                            .size(11.0).color(color));
                    }
                    "added" => {
                        ui.label(egui::RichText::new(format!("Added in \"{}\"", ch.section_name))
                            .size(11.0).color(color));
                    }
                    "removed" => {
                        ui.label(egui::RichText::new(format!("Removed from \"{}\"", ch.section_name))
                            .size(11.0).color(color));
                    }
                    _ => {}
                }
                ui.add_space(2.0);
            }
        });

    let click = ui.interact(
        frame_resp.response.rect,
        egui::Id::new(("dh_change", ch.id)),
        egui::Sense::click(),
    );
    let hovered = click.hovered();
    let clicked = click.clicked();
    if hovered { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    (hovered, clicked)
}

impl DiffHistoryScreen {
    pub fn show(&mut self, ui: &mut Ui) -> Option<DiffHistoryEvent> {
        let mut event = None;
        let mut new_selected = self.selected_group;
        let mut new_compare_a = self.compare_a;
        let mut new_compare_b = self.compare_b;
        let mut new_compare_mode = self.compare_mode;
        let mut to_toggle: Vec<i64> = vec![];

        egui::SidePanel::left("dh_left")
            .resizable(false)
            .exact_width(290.0)
            .show_inside(ui, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("← Back").clicked() {
                        event = Some(DiffHistoryEvent::Back);
                    }
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(format!("Diff History — {}", self.course_name))
                        .size(13.0).strong());
                });
                ui.add_space(6.0);
                ui.separator();

                if self.groups.is_empty() {
                    ui.add_space(20.0);
                    ui.centered_and_justified(|ui| {
                        ui.label(egui::RichText::new("No changes recorded yet.")
                            .color(ui.visuals().weak_text_color()));
                    });
                    return;
                }

                // ── Timeline strip ────────────────────────────────────────────
                let n = self.groups.len();
                const NODE_SPACING: f32 = 56.0;
                const NODE_RADIUS: f32 = 7.0;
                const STRIP_H: f32 = 64.0;
                let strip_w = n as f32 * NODE_SPACING + 20.0;

                ui.label(egui::RichText::new("Timeline").size(10.5)
                    .color(ui.visuals().weak_text_color()));
                egui::ScrollArea::horizontal()
                    .id_salt("dh_timeline_scroll")
                    .max_height(STRIP_H + 4.0)
                    .show(ui, |ui| {
                        let (rect, resp) = ui.allocate_exact_size(
                            egui::vec2(strip_w.max(ui.available_width()), STRIP_H),
                            egui::Sense::click(),
                        );
                        let painter = ui.painter_at(rect);

                        for i in 0..n {
                            let cx = rect.left() + 10.0 + i as f32 * NODE_SPACING + NODE_SPACING * 0.5;
                            let cy = rect.top() + 24.0;

                            if i + 1 < n {
                                let nx = rect.left() + 10.0 + (i + 1) as f32 * NODE_SPACING + NODE_SPACING * 0.5;
                                painter.line_segment(
                                    [egui::pos2(cx, cy), egui::pos2(nx, cy)],
                                    egui::Stroke::new(1.5, egui::Color32::from_gray(80)),
                                );
                            }

                            let color = if Some(i) == self.selected_group {
                                egui::Color32::from_rgb(255, 200, 60)
                            } else if Some(i) == self.compare_a {
                                egui::Color32::from_rgb(80, 140, 255)
                            } else if Some(i) == self.compare_b {
                                egui::Color32::from_rgb(80, 200, 80)
                            } else {
                                egui::Color32::from_gray(100)
                            };

                            let radius = if Some(i) == self.selected_group { NODE_RADIUS + 2.0 } else { NODE_RADIUS };
                            painter.circle_filled(egui::pos2(cx, cy), radius, color);
                            painter.circle_stroke(egui::pos2(cx, cy), radius,
                                egui::Stroke::new(1.0, egui::Color32::from_gray(160)));

                            let date_text = fmt_date(self.groups[i].detected_at);
                            painter.text(
                                egui::pos2(cx, cy + NODE_RADIUS + 10.0),
                                egui::Align2::CENTER_TOP,
                                &date_text,
                                egui::FontId::proportional(8.5),
                                ui.visuals().weak_text_color(),
                            );
                        }

                        if resp.clicked() {
                            if let Some(pos) = resp.interact_pointer_pos() {
                                let rel_x = pos.x - rect.left() - 10.0;
                                let idx = ((rel_x / NODE_SPACING) as usize).min(n.saturating_sub(1));
                                new_selected = Some(idx);
                            }
                        }
                    });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);

                // ── Snapshot list ─────────────────────────────────────────────
                egui::ScrollArea::vertical()
                    .id_salt("dh_list_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (i, group) in self.groups.iter().enumerate().rev() {
                            let is_selected = Some(i) == self.selected_group;
                            let frame_fill = if is_selected {
                                egui::Color32::from_rgba_premultiplied(60, 50, 20, 80)
                            } else {
                                ui.visuals().faint_bg_color
                            };

                            let card = egui::Frame::none()
                                .fill(frame_fill)
                                .rounding(5.0)
                                .inner_margin(egui::Margin::symmetric(8.0, 5.0))
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(format!("[{}]", group.hash))
                                            .monospace().size(11.0)
                                            .color(egui::Color32::from_rgb(255, 200, 60)));
                                        ui.label(egui::RichText::new(fmt_ts(group.detected_at))
                                            .size(10.5).color(ui.visuals().weak_text_color()));
                                    });
                                    ui.label(egui::RichText::new(
                                        format!("{} change{}", group.changes.len(),
                                            if group.changes.len() == 1 { "" } else { "s" }))
                                        .size(10.5).color(ui.visuals().weak_text_color()));

                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new("Compare A").size(9.5)
                                            .color(egui::Color32::from_rgb(80, 140, 255)));
                                        let a_checked = self.compare_a == Some(i);
                                        if ui.checkbox(&mut { a_checked }, "").changed() {
                                            new_compare_a = if a_checked { None } else { Some(i) };
                                        }
                                        ui.label(egui::RichText::new("B").size(9.5)
                                            .color(egui::Color32::from_rgb(80, 200, 80)));
                                        let b_checked = self.compare_b == Some(i);
                                        if ui.checkbox(&mut { b_checked }, "").changed() {
                                            new_compare_b = if b_checked { None } else { Some(i) };
                                        }
                                    });
                                });

                            let click = ui.interact(
                                card.response.rect,
                                egui::Id::new(("dh_card", i)),
                                egui::Sense::click(),
                            );
                            if click.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if click.clicked() {
                                new_selected = Some(i);
                            }
                            ui.add_space(3.0);
                        }

                        // Compare button
                        if self.compare_a.is_some() && self.compare_b.is_some()
                            && self.compare_a != self.compare_b
                        {
                            ui.add_space(4.0);
                            if ui.button("Compare selected snapshots").clicked() {
                                new_compare_mode = true;
                                new_selected = self.compare_a;
                            }
                            if self.compare_mode {
                                if ui.small_button("Exit compare").clicked() {
                                    new_compare_mode = false;
                                }
                            }
                        }
                    });
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if self.compare_mode {
                if let (Some(ai), Some(bi)) = (self.compare_a, self.compare_b) {
                    if ai < self.groups.len() && bi < self.groups.len() {
                        let (a, b) = if ai <= bi {
                            (&self.groups[ai], &self.groups[bi])
                        } else {
                            (&self.groups[bi], &self.groups[ai])
                        };
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(
                            format!("Comparing [{}] ({})  vs  [{}] ({})",
                                a.hash, fmt_ts(a.detected_at),
                                b.hash, fmt_ts(b.detected_at)))
                            .size(12.0).strong());
                        ui.separator();
                        ui.add_space(4.0);

                        ui.columns(2, |cols| {
                            cols[0].label(egui::RichText::new(
                                format!("[{}]  {}", a.hash, fmt_ts(a.detected_at)))
                                .size(11.0).strong()
                                .color(egui::Color32::from_rgb(80, 140, 255)));
                            cols[0].separator();
                            for ch in &a.changes {
                                let color = change_color(&ch.change_type);
                                cols[0].horizontal(|ui| {
                                    ui.label(egui::RichText::new(change_icon(&ch.change_type))
                                        .color(color).size(11.0));
                                    ui.label(egui::RichText::new(&ch.module_name).size(11.0));
                                });
                                if ch.change_type == "description_updated" {
                                    render_diff_block(&mut cols[0], &ch.old_val, &ch.new_val);
                                }
                            }

                            cols[1].label(egui::RichText::new(
                                format!("[{}]  {}", b.hash, fmt_ts(b.detected_at)))
                                .size(11.0).strong()
                                .color(egui::Color32::from_rgb(80, 200, 80)));
                            cols[1].separator();
                            for ch in &b.changes {
                                let color = change_color(&ch.change_type);
                                cols[1].horizontal(|ui| {
                                    ui.label(egui::RichText::new(change_icon(&ch.change_type))
                                        .color(color).size(11.0));
                                    ui.label(egui::RichText::new(&ch.module_name).size(11.0));
                                });
                                if ch.change_type == "description_updated" {
                                    render_diff_block(&mut cols[1], &ch.old_val, &ch.new_val);
                                }
                            }
                        });
                        return;
                    }
                }
            }

            if let Some(idx) = self.selected_group {
                if idx < self.groups.len() {
                    let group = &self.groups[idx];
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("[{}]", group.hash))
                            .monospace().size(13.0)
                            .color(egui::Color32::from_rgb(255, 200, 60)));
                        ui.label(egui::RichText::new(fmt_ts(group.detected_at))
                            .size(12.0).color(ui.visuals().weak_text_color()));
                    });
                    ui.label(egui::RichText::new(
                        format!("{} change{}", group.changes.len(),
                            if group.changes.len() == 1 { "" } else { "s" }))
                        .size(11.0).color(ui.visuals().weak_text_color()));
                    ui.separator();
                    ui.add_space(4.0);

                    egui::ScrollArea::vertical()
                        .id_salt("dh_detail_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for ch in &group.changes {
                                let is_expanded = self.expanded_changes.contains(&ch.id);
                                let (_, clicked) = render_change_entry(ui, ch, is_expanded);
                                if clicked { to_toggle.push(ch.id); }
                                ui.add_space(2.0);
                            }
                        });
                }
            } else {
                ui.add_space(40.0);
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new("Select a snapshot to view its changes.")
                        .color(ui.visuals().weak_text_color()));
                });
            }
        });

        self.selected_group = new_selected;
        self.compare_a = new_compare_a;
        self.compare_b = new_compare_b;
        self.compare_mode = new_compare_mode;
        for id in to_toggle {
            if self.expanded_changes.contains(&id) {
                self.expanded_changes.remove(&id);
            } else {
                self.expanded_changes.insert(id);
            }
        }

        event
    }
}

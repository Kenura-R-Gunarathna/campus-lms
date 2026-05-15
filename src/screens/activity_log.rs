use egui::Ui;
use crate::log::{LogEntry, LogLevel};

const MAX_VISIBLE: usize = 500;

pub struct ActivityLogScreen {
    pub filter: Option<&'static str>,  // None = all categories
}

impl Default for ActivityLogScreen {
    fn default() -> Self { Self { filter: None } }
}

impl ActivityLogScreen {
    pub fn show(&mut self, ui: &mut Ui, entries: &[LogEntry]) {
        let mut new_filter = self.filter;

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Activity Log").size(14.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(format!("{} entries", entries.len()))
                    .size(10.0)
                    .color(ui.visuals().weak_text_color()));
            });
        });
        ui.add_space(4.0);

        // Category filter buttons
        ui.horizontal_wrapped(|ui| {
            let categories: &[(&str, &str)] = &[
                ("all",          "All"),
                ("diff",         "Diff"),
                ("notification", "Notifications"),
                ("upload",       "Uploads"),
                ("download",     "Downloads"),
                ("auth",         "Auth"),
                ("system",       "System"),
            ];
            for &(cat, label) in categories {
                let active = if cat == "all" {
                    self.filter.is_none()
                } else {
                    self.filter == Some(cat)
                };
                if ui.selectable_label(active, label).clicked() {
                    new_filter = if cat == "all" { None } else { Some(cat) };
                }
            }
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(2.0);

        // Filtered + most-recent-first slice
        let filtered: Vec<&LogEntry> = entries.iter().rev()
            .filter(|e| self.filter.map_or(true, |f| e.category == f))
            .take(MAX_VISIBLE)
            .collect();

        if filtered.is_empty() {
            ui.add_space(40.0);
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new("No log entries yet.")
                    .color(ui.visuals().weak_text_color()));
            });
        } else {
            egui::ScrollArea::vertical()
                .id_salt("al_scroll")
                .auto_shrink([false, false])
                .stick_to_bottom(false)
                .show(ui, |ui| {
                    for entry in &filtered {
                        render_entry(ui, entry);
                    }
                });
        }

        self.filter = new_filter;
    }
}

fn render_entry(ui: &mut Ui, entry: &LogEntry) {
    let (level_color, level_char) = match entry.level {
        LogLevel::Info    => (egui::Color32::from_rgb(120, 180, 255), "·"),
        LogLevel::Success => (egui::Color32::from_rgb(100, 220, 100), "✓"),
        LogLevel::Warning => (egui::Color32::from_rgb(255, 200, 60),  "!"),
        LogLevel::Error   => (egui::Color32::from_rgb(220, 80, 80),   "✕"),
    };

    let cat_color = category_color(entry.category);

    egui::Frame::none()
        .inner_margin(egui::Margin::symmetric(4.0, 2.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                // Timestamp
                ui.label(egui::RichText::new(fmt_ts(entry.timestamp))
                    .size(9.5).monospace()
                    .color(ui.visuals().weak_text_color()));

                // Level indicator
                ui.label(egui::RichText::new(level_char)
                    .size(11.0).color(level_color));

                // Category badge
                ui.label(egui::RichText::new(format!("[{}]", entry.category))
                    .size(9.5).monospace().color(cat_color));

                // Message
                ui.label(egui::RichText::new(&entry.message).size(11.0));
            });
        });
}

fn category_color(cat: &str) -> egui::Color32 {
    match cat {
        "diff"         => egui::Color32::from_rgb(200, 160, 255),
        "notification" => egui::Color32::from_rgb(255, 200, 60),
        "upload"       => egui::Color32::from_rgb(100, 200, 255),
        "download"     => egui::Color32::from_rgb(150, 220, 150),
        "auth"         => egui::Color32::from_rgb(255, 150, 100),
        "system"       => egui::Color32::from_rgb(160, 160, 160),
        _              => egui::Color32::GRAY,
    }
}

fn fmt_ts(ts: i64) -> String {
    use chrono::{DateTime, Utc};
    DateTime::<Utc>::from_timestamp(ts, 0)
        .map(|dt| dt.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| ts.to_string())
}

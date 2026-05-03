use egui::Ui;
use chrono::{Datelike, NaiveDate, Utc};
use crate::api::types::CalendarEvent;

fn days_in_month(year: i32, month: u32) -> u32 {
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    };
    next.unwrap().pred_opt().unwrap().day()
}

fn month_name(m: u32) -> &'static str {
    ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"][(m-1) as usize]
}

fn event_color(ev: &CalendarEvent) -> egui::Color32 {
    match ev.modulename.as_deref() {
        Some("assign") => egui::Color32::from_rgb(220, 100, 60),
        Some("quiz")   => egui::Color32::from_rgb(60, 140, 220),
        Some("forum")  => egui::Color32::from_rgb(80, 180, 80),
        _              => egui::Color32::from_rgb(150, 120, 200),
    }
}

fn decode_html(s: &str) -> String {
    s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
     .replace("&quot;", "\"").replace("&#39;", "'").replace("&nbsp;", " ")
}

pub struct CalendarScreen {
    pub events: Vec<CalendarEvent>,
    pub year: i32,
    pub month: u32,
    pub selected_day: Option<u32>,
    pub detail_event: Option<CalendarEvent>,
}

impl Default for CalendarScreen {
    fn default() -> Self {
        let today = Utc::now().date_naive();
        Self {
            events: vec![],
            year: today.year(),
            month: today.month(),
            selected_day: Some(today.day()),
            detail_event: None,
        }
    }
}

impl CalendarScreen {
    pub fn show(&mut self, ui: &mut Ui) {
        let today = Utc::now().date_naive();

        egui::TopBottomPanel::top("cal_topbar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Calendar").size(16.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("▶").clicked() {
                        if self.month == 12 { self.month = 1; self.year += 1; }
                        else { self.month += 1; }
                        self.selected_day = None;
                    }
                    ui.label(egui::RichText::new(
                        format!("{} {}", month_name(self.month), self.year)).size(14.0).strong());
                    if ui.small_button("◀").clicked() {
                        if self.month == 1 { self.month = 12; self.year -= 1; }
                        else { self.month -= 1; }
                        self.selected_day = None;
                    }
                });
            });
        });

        // Right panel: event detail or day event list
        egui::SidePanel::right("cal_detail")
            .resizable(true)
            .default_width(260.0)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if let Some(ev) = &self.detail_event.clone() {
                        // Full event detail
                        if ui.small_button("← Back").clicked() {
                            self.detail_event = None;
                        }
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(decode_html(&ev.name)).size(15.0).strong());
                        ui.add_space(6.0);

                        if ev.timestart > 0 {
                            let dt: chrono::DateTime<chrono::Local> =
                                chrono::DateTime::from(chrono::DateTime::<Utc>::from_timestamp(ev.timestart, 0).unwrap());
                            ui.label(egui::RichText::new(dt.format("📅 %A, %d %b %Y  %H:%M").to_string()).size(13.0));
                        }
                        if let Some(course) = &ev.coursename {
                            ui.label(egui::RichText::new(format!("📚 {course}")).size(12.0)
                                .color(ui.visuals().weak_text_color()));
                        }
                        if let Some(m) = &ev.modulename {
                            ui.label(egui::RichText::new(format!("🔧 {m}")).size(12.0)
                                .color(ui.visuals().weak_text_color()));
                        }
                        if let Some(desc) = &ev.description {
                            let clean = decode_html(desc);
                            // Strip any remaining HTML tags simply
                            let plain: String = clean.chars().fold((String::new(), false), |(mut acc, in_tag), c| {
                                if c == '<' { (acc, true) }
                                else if c == '>' { (acc, false) }
                                else if !in_tag { acc.push(c); (acc, false) }
                                else { (acc, in_tag) }
                            }).0;
                            if !plain.trim().is_empty() {
                                ui.add_space(8.0);
                                ui.label(egui::RichText::new(plain.trim()).size(12.0));
                            }
                        }
                    } else {
                        // Day event list
                        let day_label = if let Some(d) = self.selected_day {
                            format!("{} {} {}", d, month_name(self.month), self.year)
                        } else {
                            "Select a day".into()
                        };
                        ui.label(egui::RichText::new(day_label).size(14.0).strong());
                        ui.separator();

                        if let Some(day) = self.selected_day {
                            let day_events: Vec<&CalendarEvent> = self.events.iter()
                                .filter(|e| {
                                    let dt = chrono::DateTime::<Utc>::from_timestamp(e.timestart, 0)
                                        .unwrap().date_naive();
                                    dt.year() == self.year && dt.month() == self.month && dt.day() == day
                                })
                                .collect();

                            if day_events.is_empty() {
                                ui.label(egui::RichText::new("No events").color(ui.visuals().weak_text_color()));
                            }

                            for ev in day_events {
                                ui.add_space(4.0);
                                let color = event_color(ev);
                                if egui::Frame::none()
                                    .fill(ui.visuals().faint_bg_color)
                                    .rounding(4.0)
                                    .inner_margin(8.0)
                                    .show(ui, |ui| {
                                        ui.colored_label(color, "●");
                                        ui.label(egui::RichText::new(decode_html(&ev.name)).size(13.0));
                                        if let Some(c) = &ev.coursename {
                                            ui.label(egui::RichText::new(c).size(11.0)
                                                .color(ui.visuals().weak_text_color()));
                                        }
                                    }).response.interact(egui::Sense::click()).clicked()
                                {
                                    self.detail_event = Some(ev.clone());
                                }
                            }
                        } else {
                            // Show upcoming events summary
                            let now_ts = Utc::now().timestamp();
                            let mut upcoming: Vec<&CalendarEvent> = self.events.iter()
                                .filter(|e| e.timestart >= now_ts)
                                .take(10)
                                .collect();
                            upcoming.sort_by_key(|e| e.timestart);

                            if upcoming.is_empty() && !self.events.is_empty() {
                                ui.label(egui::RichText::new("No upcoming events")
                                    .color(ui.visuals().weak_text_color()));
                            }
                            if self.events.is_empty() {
                                ui.spinner();
                            }

                            for ev in upcoming {
                                ui.add_space(4.0);
                                let dt: chrono::DateTime<chrono::Local> =
                                    chrono::DateTime::from(chrono::DateTime::<Utc>::from_timestamp(ev.timestart, 0).unwrap());
                                let color = event_color(ev);
                                if egui::Frame::none()
                                    .fill(ui.visuals().faint_bg_color)
                                    .rounding(4.0)
                                    .inner_margin(8.0)
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.colored_label(color, "●");
                                            ui.label(egui::RichText::new(decode_html(&ev.name)).size(13.0));
                                        });
                                        ui.label(egui::RichText::new(dt.format("%d %b, %H:%M").to_string())
                                            .size(11.0).color(ui.visuals().weak_text_color()));
                                    }).response.interact(egui::Sense::click()).clicked()
                                {
                                    self.detail_event = Some(ev.clone());
                                }
                            }
                        }
                    }
                });
            });

        // Month grid
        egui::CentralPanel::default().show_inside(ui, |ui| {
            let dim = days_in_month(self.year, self.month);
            let first_day = NaiveDate::from_ymd_opt(self.year, self.month, 1).unwrap();
            // ISO weekday: Mon=0 .. Sun=6
            let start_offset = first_day.weekday().num_days_from_monday() as u32;

            let cell_w = (ui.available_width() / 7.0).floor();
            let cell_h = 72.0_f32;

            // Day headers
            ui.horizontal(|ui| {
                for day in ["Mon","Tue","Wed","Thu","Fri","Sat","Sun"] {
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(cell_w, 22.0), egui::Sense::hover());
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        day,
                        egui::FontId::proportional(12.0),
                        ui.visuals().weak_text_color(),
                    );
                }
            });

            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let mut col = start_offset;
                let mut row_start = true;
                let mut row_rect_start = ui.cursor().min;

                if start_offset > 0 {
                    // Allocate empty cells for offset
                    ui.horizontal(|ui| {
                        for _ in 0..start_offset {
                            ui.allocate_exact_size(egui::vec2(cell_w, cell_h), egui::Sense::hover());
                        }
                        row_start = false;
                    });
                }

                let mut day_ui_cells: Vec<(u32, egui::Rect)> = Vec::new();

                // We need to lay out cells row by row
                let mut current_col = start_offset;
                let mut rows: Vec<Vec<Option<u32>>> = Vec::new();
                let mut current_row: Vec<Option<u32>> = (0..start_offset).map(|_| None).collect();

                for d in 1..=dim {
                    current_row.push(Some(d));
                    current_col += 1;
                    if current_col == 7 {
                        rows.push(current_row.clone());
                        current_row = Vec::new();
                        current_col = 0;
                    }
                }
                if !current_row.is_empty() {
                    while current_row.len() < 7 { current_row.push(None); }
                    rows.push(current_row);
                }

                let _ = col;
                let _ = row_start;
                let _ = row_rect_start;

                for row in rows {
                    ui.horizontal(|ui| {
                        for cell in row {
                            match cell {
                                None => { ui.allocate_exact_size(egui::vec2(cell_w, cell_h), egui::Sense::hover()); }
                                Some(d) => {
                                    let is_today = today.year() == self.year
                                        && today.month() == self.month
                                        && today.day() == d;
                                    let is_selected = self.selected_day == Some(d);

                                    // Count events on this day
                                    let day_events: Vec<&CalendarEvent> = self.events.iter()
                                        .filter(|e| {
                                            let dt = chrono::DateTime::<Utc>::from_timestamp(e.timestart, 0)
                                                .unwrap().date_naive();
                                            dt.year() == self.year && dt.month() == self.month && dt.day() == d
                                        })
                                        .collect();

                                    let (rect, resp) = ui.allocate_exact_size(
                                        egui::vec2(cell_w, cell_h), egui::Sense::click());

                                    let bg = if is_selected {
                                        egui::Color32::from_rgb(40, 60, 90)
                                    } else if resp.hovered() {
                                        ui.visuals().faint_bg_color
                                    } else {
                                        egui::Color32::TRANSPARENT
                                    };

                                    ui.painter().rect_filled(rect, 4.0, bg);

                                    // Day number
                                    let num_color = if is_today {
                                        egui::Color32::from_rgb(80, 150, 255)
                                    } else {
                                        ui.visuals().text_color()
                                    };

                                    ui.painter().text(
                                        rect.min + egui::vec2(6.0, 4.0),
                                        egui::Align2::LEFT_TOP,
                                        d.to_string(),
                                        egui::FontId::proportional(if is_today { 14.0 } else { 13.0 }),
                                        num_color,
                                    );

                                    // Event dots (max 4)
                                    let dot_y = rect.min.y + 22.0;
                                    let mut dot_x = rect.min.x + 5.0;
                                    for ev in day_events.iter().take(4) {
                                        let color = event_color(ev);
                                        ui.painter().circle_filled(
                                            egui::pos2(dot_x, dot_y), 4.0, color);
                                        dot_x += 11.0;
                                    }
                                    if day_events.len() > 4 {
                                        ui.painter().text(
                                            egui::pos2(dot_x, dot_y),
                                            egui::Align2::LEFT_CENTER,
                                            format!("+{}", day_events.len() - 4),
                                            egui::FontId::proportional(9.0),
                                            ui.visuals().weak_text_color(),
                                        );
                                    }

                                    // Event name preview (first event)
                                    if let Some(ev) = day_events.first() {
                                        let preview = decode_html(&ev.name);
                                        let short: String = preview.chars().take(14).collect();
                                        let label = if preview.len() > 14 { format!("{short}…") } else { short };
                                        ui.painter().text(
                                            rect.min + egui::vec2(5.0, 34.0),
                                            egui::Align2::LEFT_TOP,
                                            label,
                                            egui::FontId::proportional(9.5),
                                            event_color(ev),
                                        );
                                    }

                                    if resp.clicked() {
                                        self.selected_day = Some(d);
                                        self.detail_event = None;
                                    }
                                }
                            }
                        }
                    });
                }
            });
        });
    }
}

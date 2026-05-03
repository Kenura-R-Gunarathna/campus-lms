use egui::Ui;
use chrono::{Datelike, NaiveDate, Utc};
use crate::api::types::CalendarEvent;
use crate::models::decode_html;

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
        Some("assign")       => egui::Color32::from_rgb(220, 100, 60),
        Some("quiz")         => egui::Color32::from_rgb(60, 140, 220),
        Some("forum")        => egui::Color32::from_rgb(80, 180, 80),
        Some("announcement") => egui::Color32::from_rgb(80, 180, 80),
        Some("personal")     => egui::Color32::from_rgb(200, 150, 50),
        _                    => egui::Color32::from_rgb(150, 120, 200),
    }
}

fn get_event_time(ev: &CalendarEvent) -> i64 {
    if ev.timesort > 0 { ev.timesort } else { ev.timestart }
}

fn fmt_event_time(ts: i64) -> (String, Option<&'static str>) {
    let dt: chrono::DateTime<chrono::Local> =
        chrono::DateTime::from(chrono::DateTime::<Utc>::from_timestamp(ts, 0).unwrap());
    let base = dt.format("%A, %d %b %Y  %H:%M").to_string();
    use chrono::Timelike;
    let note = if dt.hour() == 0 && dt.minute() == 0 {
        Some("midnight = start of this day, not end of previous")
    } else if dt.hour() == 23 && dt.minute() == 59 {
        Some("11:59 PM = end of this day")
    } else {
        None
    };
    (base, note)
}

#[derive(PartialEq, Clone, Copy)]
pub enum CalendarView { Month, Week, Agenda }

pub enum CalendarScreenEvent {
    DeletePersonal(u64),
}

pub struct CalendarScreen {
    pub events: Vec<CalendarEvent>,
    pub personal_events: Vec<CalendarEvent>,
    pub assignment_events: Vec<CalendarEvent>,
    pub announcement_events: Vec<CalendarEvent>,
    pub year: i32,
    pub month: u32,
    pub selected_day: Option<u32>,
    pub detail_event: Option<CalendarEvent>,
    pub showing_add_event: bool,
    pub new_event_name: String,
    pub show_personal_events: bool,
    pub view: CalendarView,
    // week view: which week's Monday (by NaiveDate)
    week_start: NaiveDate,
}

impl Default for CalendarScreen {
    fn default() -> Self {
        let today = Utc::now().date_naive();
        let monday = today - chrono::Duration::days(
            today.weekday().num_days_from_monday() as i64);
        Self {
            events: vec![],
            personal_events: vec![],
            assignment_events: vec![],
            announcement_events: vec![],
            year: today.year(),
            month: today.month(),
            selected_day: Some(today.day()),
            detail_event: None,
            showing_add_event: false,
            new_event_name: String::new(),
            show_personal_events: true,
            view: CalendarView::Month,
            week_start: monday,
        }
    }
}

impl CalendarScreen {
    fn all_events(&self) -> Vec<CalendarEvent> {
        let mut v: Vec<CalendarEvent> = self.events.iter().cloned()
            .chain(self.assignment_events.iter().cloned())
            .chain(self.announcement_events.iter().cloned())
            .collect();
        if self.show_personal_events {
            v.extend(self.personal_events.iter().cloned());
        }
        v
    }

    fn events_on_date(&self, date: NaiveDate) -> Vec<CalendarEvent> {
        self.all_events().into_iter().filter(|e| {
            let ts = get_event_time(e);
            if ts == 0 { return false; }
            let d = chrono::DateTime::<Utc>::from_timestamp(ts, 0)
                .unwrap().date_naive();
            d == date
        }).collect()
    }

    pub fn show(&mut self, ui: &mut Ui) -> Option<CalendarScreenEvent> {
        let mut ret_event = None;
        let today = Utc::now().date_naive();

        // ── Top bar ─────────────────────────────────────────────────────────
        egui::TopBottomPanel::top("cal_topbar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Calendar").size(16.0).strong());
                if ui.button("+ Add Activity").clicked() {
                    self.showing_add_event = !self.showing_add_event;
                }
                ui.separator();
                ui.checkbox(&mut self.show_personal_events, "Show personal");
                ui.separator();
                // View mode switcher
                for (v, label) in [
                    (CalendarView::Month, "Month"),
                    (CalendarView::Week,  "Week"),
                    (CalendarView::Agenda,"Agenda"),
                ] {
                    if ui.selectable_label(self.view == v, label).clicked() {
                        self.view = v;
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    match self.view {
                        CalendarView::Month => {
                            if ui.small_button(egui_phosphor::regular::CARET_RIGHT).clicked() {
                                if self.month == 12 { self.month = 1; self.year += 1; }
                                else { self.month += 1; }
                                self.selected_day = None;
                            }
                            ui.label(egui::RichText::new(
                                format!("{} {}", month_name(self.month), self.year)).size(14.0).strong());
                            if ui.small_button(egui_phosphor::regular::CARET_LEFT).clicked() {
                                if self.month == 1 { self.month = 12; self.year -= 1; }
                                else { self.month -= 1; }
                                self.selected_day = None;
                            }
                        }
                        CalendarView::Week => {
                            if ui.small_button(egui_phosphor::regular::CARET_RIGHT).clicked() {
                                self.week_start += chrono::Duration::days(7);
                            }
                            let week_end = self.week_start + chrono::Duration::days(6);
                            ui.label(egui::RichText::new(
                                format!("{} {} – {} {}",
                                    self.week_start.day(), month_name(self.week_start.month()),
                                    week_end.day(), month_name(week_end.month())
                                )).size(14.0).strong());
                            if ui.small_button(egui_phosphor::regular::CARET_LEFT).clicked() {
                                self.week_start -= chrono::Duration::days(7);
                            }
                        }
                        CalendarView::Agenda => {
                            ui.label(egui::RichText::new("Upcoming events").size(14.0).strong());
                        }
                    }
                });
            });
        });

        // ── Right panel: event detail ────────────────────────────────────────
        if self.view != CalendarView::Agenda {
            egui::SidePanel::right("cal_detail")
                .resizable(true)
                .default_width(260.0)
                .show_inside(ui, |ui| {
                    ret_event = self.render_detail_panel(ui, today);
                });
        }

        // ── Main area ────────────────────────────────────────────────────────
        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.show_add_event_modal(ui);
            match self.view {
                CalendarView::Month  => self.render_month(ui, today),
                CalendarView::Week   => self.render_week(ui, today),
                CalendarView::Agenda => { ret_event = self.render_agenda(ui, today); }
            }
        });

        ret_event
    }

    fn render_detail_panel(&mut self, ui: &mut Ui, today: NaiveDate) -> Option<CalendarScreenEvent> {
        let mut ret = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            if let Some(ev) = &self.detail_event.clone() {
                if ui.small_button("← Back").clicked() { self.detail_event = None; }
                ui.add_space(6.0);
                ui.label(egui::RichText::new(decode_html(&ev.name)).size(15.0).strong());
                ui.add_space(6.0);
                let ts = get_event_time(ev);
                if ts > 0 {
                    let (time_str, note) = fmt_event_time(ts);
                    ui.label(egui::RichText::new(time_str).size(13.0));
                    if let Some(n) = note {
                        ui.label(egui::RichText::new(format!("! {n}"))
                            .size(11.0).color(egui::Color32::from_rgb(255, 200, 80)));
                    }
                }
                if let Some(course) = &ev.coursename {
                    ui.label(egui::RichText::new(format!("Course: {course}")).size(12.0)
                        .color(ui.visuals().weak_text_color()));
                }
                if let Some(m) = &ev.modulename {
                    ui.label(egui::RichText::new(format!("Type: {m}")).size(12.0)
                        .color(ui.visuals().weak_text_color()));
                }
                if let Some(desc) = &ev.description {
                    let plain = strip_html(&decode_html(desc));
                    if !plain.trim().is_empty() {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(plain.trim()).size(12.0));
                    }
                }
                if ev.eventtype.as_deref() == Some("user") {
                    ui.add_space(10.0);
                    if ui.button("Delete Activity").clicked() {
                        ret = Some(CalendarScreenEvent::DeletePersonal(ev.id));
                        self.detail_event = None;
                    }
                }
            } else {
                let day_label = if let Some(d) = self.selected_day {
                    format!("{} {} {}", d, month_name(self.month), self.year)
                } else {
                    "Select a day".into()
                };
                ui.label(egui::RichText::new(day_label).size(14.0).strong());
                ui.separator();

                if let Some(day) = self.selected_day {
                    let date = NaiveDate::from_ymd_opt(self.year, self.month, day).unwrap();
                    let day_events = self.events_on_date(date);
                    if day_events.is_empty() {
                        ui.label(egui::RichText::new("No events")
                            .color(ui.visuals().weak_text_color()));
                    }
                    for ev in &day_events {
                        ui.add_space(4.0);
                        let color = event_color(ev);
                        let inner = egui::Frame::none()
                            .fill(ui.visuals().faint_bg_color)
                            .rounding(4.0).inner_margin(8.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (dot_rect, _) = ui.allocate_exact_size(
                                        egui::vec2(10.0, 10.0), egui::Sense::hover());
                                    ui.painter().circle_filled(dot_rect.center(), 4.0, color);
                                    ui.label(egui::RichText::new(decode_html(&ev.name)).size(13.0));
                                });
                                if let Some(c) = &ev.coursename {
                                    ui.label(egui::RichText::new(c).size(11.0)
                                        .color(ui.visuals().weak_text_color()));
                                }
                            });
                        let r = ui.interact(inner.response.rect, egui::Id::new(("det_ev", ev.id)), egui::Sense::click());
                        if r.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                        if r.clicked() { self.detail_event = Some(ev.clone()); }
                    }
                } else {
                    // Upcoming events
                    let now_ts = Utc::now().timestamp();
                    let mut upcoming: Vec<CalendarEvent> = self.all_events()
                        .into_iter().filter(|e| get_event_time(e) >= now_ts)
                        .collect();
                    upcoming.sort_by_key(|e| get_event_time(e));
                    upcoming.truncate(15);
                    if upcoming.is_empty() {
                        let all = self.all_events();
                        if all.is_empty() { ui.spinner(); }
                        else { ui.label(egui::RichText::new("No upcoming events")
                            .color(ui.visuals().weak_text_color())); }
                    }
                    for ev in &upcoming {
                        ui.add_space(4.0);
                        let ts = get_event_time(ev);
                        let dt: chrono::DateTime<chrono::Local> =
                            chrono::DateTime::from(chrono::DateTime::<Utc>::from_timestamp(ts, 0).unwrap());
                        let color = event_color(ev);
                        let inner = egui::Frame::none()
                            .fill(ui.visuals().faint_bg_color)
                            .rounding(4.0).inner_margin(8.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (dot_rect, _) = ui.allocate_exact_size(
                                        egui::vec2(10.0, 10.0), egui::Sense::hover());
                                    ui.painter().circle_filled(dot_rect.center(), 4.0, color);
                                    ui.label(egui::RichText::new(decode_html(&ev.name)).size(13.0));
                                });
                                ui.label(egui::RichText::new(dt.format("%d %b, %H:%M").to_string())
                                    .size(11.0).color(ui.visuals().weak_text_color()));
                            });
                        let r = ui.interact(inner.response.rect, egui::Id::new(("upc_ev", ev.id)), egui::Sense::click());
                        if r.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                        if r.clicked() { self.detail_event = Some(ev.clone()); }
                    }
                }
            }
        });
        let _ = today; // suppress unused warning
        ret
    }

    fn render_month(&mut self, ui: &mut Ui, today: NaiveDate) {
        let dim = days_in_month(self.year, self.month);
        let first_day = NaiveDate::from_ymd_opt(self.year, self.month, 1).unwrap();
        let start_offset = first_day.weekday().num_days_from_monday() as u32;
        let cell_w = (ui.available_width() / 7.0).floor();
        let cell_h = 88.0_f32;

        // Day headers
        ui.horizontal(|ui| {
            for day in ["Mon","Tue","Wed","Thu","Fri","Sat","Sun"] {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(cell_w, 22.0), egui::Sense::hover());
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, day,
                    egui::FontId::proportional(12.0), ui.visuals().weak_text_color());
            }
        });

        // Build rows
        let mut rows: Vec<Vec<Option<u32>>> = Vec::new();
        let mut current_row: Vec<Option<u32>> = (0..start_offset).map(|_| None).collect();
        let mut col = start_offset;
        for d in 1..=dim {
            current_row.push(Some(d));
            col += 1;
            if col == 7 { rows.push(std::mem::take(&mut current_row)); col = 0; }
        }
        if !current_row.is_empty() {
            while current_row.len() < 7 { current_row.push(None); }
            rows.push(current_row);
        }

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // Highlight current week row
            let today_col = if today.year() == self.year && today.month() == self.month {
                let offset = first_day.weekday().num_days_from_monday() as u32;
                let slot = (today.day() - 1 + offset) / 7;
                Some(slot)
            } else { None };

            for (row_idx, row) in rows.iter().enumerate() {
                let row_rect_start = ui.cursor().min;
                let is_current_week_row = today_col == Some(row_idx as u32);

                ui.horizontal(|ui| {
                    for cell in row {
                        match cell {
                            None => { ui.allocate_exact_size(egui::vec2(cell_w, cell_h), egui::Sense::hover()); }
                            Some(d) => {
                                let date = NaiveDate::from_ymd_opt(self.year, self.month, *d).unwrap();
                                let is_today = date == today;
                                let is_selected = self.selected_day == Some(*d);
                                let day_events = self.events_on_date(date);

                                let (rect, resp) = ui.allocate_exact_size(egui::vec2(cell_w, cell_h), egui::Sense::click());

                                // Background
                                let bg = if is_selected {
                                    egui::Color32::from_rgb(35, 55, 85)
                                } else if is_current_week_row {
                                    egui::Color32::from_rgba_premultiplied(40, 50, 70, 60)
                                } else if resp.hovered() {
                                    ui.visuals().faint_bg_color
                                } else {
                                    egui::Color32::TRANSPARENT
                                };
                                ui.painter().rect_filled(rect, 4.0, bg);

                                // Today circle
                                let num_color = if is_today {
                                    egui::Color32::WHITE
                                } else {
                                    ui.visuals().text_color()
                                };
                                if is_today {
                                    ui.painter().circle_filled(
                                        rect.min + egui::vec2(14.0, 14.0), 11.0,
                                        egui::Color32::from_rgb(66, 133, 244));
                                }
                                ui.painter().text(
                                    rect.min + egui::vec2(14.0, 14.0),
                                    egui::Align2::CENTER_CENTER,
                                    d.to_string(),
                                    egui::FontId::proportional(13.0),
                                    num_color,
                                );

                                // Event pills (up to 3)
                                let pill_y_start = rect.min.y + 30.0;
                                let pill_h = 14.0;
                                let pill_gap = 2.0;
                                let max_pills = 3usize;
                                let show_count = day_events.len().min(max_pills);
                                for (i, ev) in day_events.iter().take(show_count).enumerate() {
                                    let pill_rect = egui::Rect::from_min_size(
                                        egui::pos2(rect.min.x + 2.0,
                                            pill_y_start + i as f32 * (pill_h + pill_gap)),
                                        egui::vec2(cell_w - 4.0, pill_h),
                                    );
                                    let color = event_color(ev);
                                    ui.painter().rect_filled(pill_rect, 2.0, color);
                                    let name = decode_html(&ev.name);
                                    let short: String = name.chars().take(12).collect();
                                    let label = if name.chars().count() > 12 { format!("{short}…") } else { short };
                                    ui.painter().text(
                                        pill_rect.min + egui::vec2(3.0, pill_h / 2.0),
                                        egui::Align2::LEFT_CENTER,
                                        label,
                                        egui::FontId::proportional(9.5),
                                        egui::Color32::WHITE,
                                    );
                                }
                                if day_events.len() > max_pills {
                                    let y = pill_y_start + max_pills as f32 * (pill_h + pill_gap);
                                    ui.painter().text(
                                        egui::pos2(rect.min.x + 4.0, y),
                                        egui::Align2::LEFT_TOP,
                                        format!("+{} more", day_events.len() - max_pills),
                                        egui::FontId::proportional(9.0),
                                        ui.visuals().weak_text_color(),
                                    );
                                }

                                if resp.clicked() {
                                    self.selected_day = Some(*d);
                                    self.detail_event = None;
                                }
                            }
                        }
                    }
                });
                let _ = row_rect_start; // available for future use
            }
        });
    }

    fn render_week(&mut self, ui: &mut Ui, today: NaiveDate) {
        let cell_w = (ui.available_width() / 7.0).floor();
        let day_names = ["Mon","Tue","Wed","Thu","Fri","Sat","Sun"];

        // Header row
        ui.horizontal(|ui| {
            for col in 0..7u32 {
                let date = self.week_start + chrono::Duration::days(col as i64);
                let is_today = date == today;
                let (rect, _) = ui.allocate_exact_size(egui::vec2(cell_w, 40.0), egui::Sense::hover());
                if is_today {
                    ui.painter().rect_filled(rect, 4.0,
                        egui::Color32::from_rgba_premultiplied(66, 133, 244, 60));
                }
                ui.painter().text(rect.center() - egui::vec2(0.0, 8.0),
                    egui::Align2::CENTER_CENTER,
                    day_names[col as usize],
                    egui::FontId::proportional(11.0),
                    ui.visuals().weak_text_color());
                let num_color = if is_today { egui::Color32::from_rgb(66, 133, 244) }
                    else { ui.visuals().text_color() };
                ui.painter().text(rect.center() + egui::vec2(0.0, 8.0),
                    egui::Align2::CENTER_CENTER,
                    date.day().to_string(),
                    egui::FontId::proportional(if is_today { 15.0 } else { 13.0 }).clone(),
                    num_color);
            }
        });
        ui.separator();

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.horizontal(|ui| {
                for col in 0..7u32 {
                    let date = self.week_start + chrono::Duration::days(col as i64);
                    let day_events = self.events_on_date(date);
                    let col_rect_start = ui.cursor().min;
                    ui.vertical(|ui| {
                        ui.set_min_width(cell_w);
                        ui.set_max_width(cell_w);
                        if day_events.is_empty() {
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new("–").size(11.0)
                                .color(ui.visuals().weak_text_color()));
                        }
                        for ev in &day_events {
                            let color = event_color(ev);
                            let inner = egui::Frame::none()
                                .fill(color.linear_multiply(0.3))
                                .rounding(4.0)
                                .inner_margin(egui::Margin::symmetric(4.0, 3.0))
                                .show(ui, |ui| {
                                    ui.set_min_width(cell_w - 8.0);
                                    ui.label(egui::RichText::new(decode_html(&ev.name))
                                        .size(11.0).color(color));
                                    if let Some(c) = &ev.coursename {
                                        ui.label(egui::RichText::new(c).size(9.5)
                                            .color(ui.visuals().weak_text_color()));
                                    }
                                });
                            let r = ui.interact(inner.response.rect,
                                egui::Id::new(("wk_ev", ev.id)), egui::Sense::click());
                            if r.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                            if r.clicked() { self.detail_event = Some(ev.clone()); }
                            ui.add_space(2.0);
                        }
                    });
                    let _ = col_rect_start;
                }
            });
        });
    }

    fn render_agenda(&mut self, ui: &mut Ui, today: NaiveDate) -> Option<CalendarScreenEvent> {
        let mut ret = None;
        let now_ts = Utc::now().timestamp();
        let mut all: Vec<CalendarEvent> = self.all_events()
            .into_iter().filter(|e| get_event_time(e) >= now_ts - 86400)
            .collect();
        all.sort_by_key(|e| get_event_time(e));
        all.dedup_by_key(|e| e.id);

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if all.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new("No upcoming events")
                        .color(ui.visuals().weak_text_color()));
                });
                return;
            }

            let mut last_date: Option<NaiveDate> = None;
            for ev in &all {
                let ts = get_event_time(ev);
                let dt: chrono::DateTime<chrono::Local> =
                    chrono::DateTime::from(chrono::DateTime::<Utc>::from_timestamp(ts, 0).unwrap());
                let date = dt.date_naive();

                if last_date != Some(date) {
                    ui.add_space(8.0);
                    let is_today = date == today;
                    let label = if is_today {
                        "Today".to_string()
                    } else {
                        format!("{} {} {}", date.weekday(), date.day(), month_name(date.month()))
                    };
                    let color = if is_today {
                        egui::Color32::from_rgb(66, 133, 244)
                    } else {
                        ui.visuals().strong_text_color()
                    };
                    ui.label(egui::RichText::new(label).size(13.0).strong().color(color));
                    ui.separator();
                    last_date = Some(date);
                }

                let color = event_color(ev);
                let inner = egui::Frame::none()
                    .fill(ui.visuals().faint_bg_color)
                    .rounding(4.0)
                    .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Color bar
                            let (bar, _) = ui.allocate_exact_size(
                                egui::vec2(4.0, 36.0), egui::Sense::hover());
                            ui.painter().rect_filled(bar, 2.0, color);
                            ui.add_space(6.0);
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(decode_html(&ev.name)).size(13.0));
                                let mut meta = dt.format("%H:%M").to_string();
                                if let Some(c) = &ev.coursename {
                                    meta = format!("{meta}  ·  {c}");
                                }
                                ui.label(egui::RichText::new(meta).size(11.0)
                                    .color(ui.visuals().weak_text_color()));
                            });
                        });
                    });
                let r = ui.interact(inner.response.rect,
                    egui::Id::new(("ag_ev", ev.id)), egui::Sense::click());
                if r.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                if r.clicked() {
                    // Show event detail inline
                    self.detail_event = Some(ev.clone());
                    self.view = CalendarView::Month; // switch to month to show detail panel
                }
                if ev.eventtype.as_deref() == Some("user") {
                    ui.add_space(2.0);
                    if ui.small_button("Delete").clicked() {
                        ret = Some(CalendarScreenEvent::DeletePersonal(ev.id));
                    }
                }
                ui.add_space(2.0);
            }
        });

        ret
    }

    fn show_add_event_modal(&mut self, ui: &mut Ui) {
        if !self.showing_add_event { return; }
        egui::Window::new("New Personal Activity")
            .collapsible(false).resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut self.new_event_name);
                let day_str = self.selected_day.map(|d| d.to_string()).unwrap_or_else(|| "none".into());
                ui.label(format!("Date: {} {} {} (select on calendar)", day_str, month_name(self.month), self.year));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        if !self.new_event_name.trim().is_empty() {
                            if let Some(day) = self.selected_day {
                                let dt = NaiveDate::from_ymd_opt(self.year, self.month, day).unwrap()
                                    .and_hms_opt(12, 0, 0).unwrap();
                                let ts = dt.and_local_timezone(chrono::Local).unwrap().timestamp();
                                self.personal_events.push(CalendarEvent {
                                    id: rand::random(),
                                    name: self.new_event_name.clone(),
                                    description: None,
                                    timestart: ts,
                                    timesort: ts,
                                    courseid: 0,
                                    coursename: Some("Personal".into()),
                                    modulename: Some("personal".into()),
                                    eventtype: Some("user".into()),
                                });
                                self.new_event_name.clear();
                                self.showing_add_event = false;
                            }
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        self.showing_add_event = false;
                        self.new_event_name.clear();
                    }
                });
            });
    }
}

fn strip_html(s: &str) -> String {
    s.chars().fold((String::new(), false), |(mut acc, in_tag), c| {
        if c == '<' { (acc, true) }
        else if c == '>' { (acc, false) }
        else if !in_tag { acc.push(c); (acc, false) }
        else { (acc, in_tag) }
    }).0
}

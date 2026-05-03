use egui::{Ui, Color32, RichText};
use crate::api::types::UserProfile;
use crate::models::{parse_student_id, year_label};

fn avatar_color(userid: u64) -> Color32 {
    // Golden-angle hash → consistent, well-distributed hue
    let hue = ((userid.wrapping_mul(2654435761)) % 360) as f32;
    let s = 0.55_f32;
    let v = 0.75_f32;
    // HSV → RGB
    let h6 = hue / 60.0;
    let i = h6.floor() as u32;
    let f = h6 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

fn info_row(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(12.0).color(ui.visuals().weak_text_color()));
        ui.label(RichText::new(value).size(13.0));
    });
    ui.add_space(3.0);
}

pub struct ProfileScreen {
    pub user: Option<UserProfile>,
    pub student_year: Option<u8>,
    pub dept_counts: Vec<(String, usize)>, // (dept, course count)
}

impl Default for ProfileScreen {
    fn default() -> Self {
        Self { user: None, student_year: None, dept_counts: vec![] }
    }
}

impl ProfileScreen {
    pub fn show(&mut self, ui: &mut Ui) {
        if self.user.is_none() {
            ui.centered_and_justified(|ui| { ui.spinner(); });
            return;
        }
        let user = self.user.as_ref().unwrap();

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(24.0);

            // ── Avatar + name ───────────────────────────────────────────────
            ui.vertical_centered(|ui| {
                // Avatar circle
                let radius = 40.0_f32;
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(radius * 2.0, radius * 2.0), egui::Sense::hover());
                let color = avatar_color(user.id);
                ui.painter().circle_filled(rect.center(), radius, color);

                let initials = format!(
                    "{}{}",
                    user.firstname.chars().next().unwrap_or('?'),
                    user.lastname.chars().next().unwrap_or('?')
                ).to_uppercase();
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &initials,
                    egui::FontId::proportional(22.0),
                    Color32::WHITE,
                );

                ui.add_space(12.0);
                ui.label(RichText::new(&user.fullname).size(22.0).strong());

                // Student ID badge
                if let Some(sid) = parse_student_id(&user.email) {
                    ui.add_space(4.0);
                    egui::Frame::none()
                        .fill(Color32::from_rgb(40, 60, 90))
                        .rounding(12.0)
                        .inner_margin(egui::Margin::symmetric(12.0, 4.0))
                        .show(ui, |ui| {
                            ui.label(RichText::new(&sid).size(13.0)
                                .color(Color32::from_rgb(140, 190, 255)));
                        });
                }

                // Year badge
                if let Some(y) = self.student_year {
                    ui.add_space(4.0);
                    egui::Frame::none()
                        .fill(Color32::from_rgb(30, 70, 50))
                        .rounding(12.0)
                        .inner_margin(egui::Margin::symmetric(12.0, 4.0))
                        .show(ui, |ui| {
                            ui.label(RichText::new(year_label(y)).size(12.0)
                                .color(Color32::from_rgb(100, 210, 130)));
                        });
                }
            });

            ui.add_space(24.0);
            ui.separator();
            ui.add_space(12.0);

            // ── Info fields ────────────────────────────────────────────────
            let card_w = (ui.available_width() - 60.0).min(500.0);
            ui.vertical_centered(|ui| {
                ui.set_max_width(card_w);

                egui::Frame::none()
                    .fill(ui.visuals().faint_bg_color)
                    .rounding(8.0)
                    .inner_margin(egui::Margin::symmetric(20.0, 16.0))
                    .show(ui, |ui| {
                        ui.set_min_width(card_w);
                        ui.label(RichText::new("Academic Info").size(14.0).strong());
                        ui.separator();
                        ui.add_space(6.0);
                        info_row(ui, "Email", &user.email);
                        if let Some(idn) = &user.idnumber {
                            info_row(ui, "ID Number", idn);
                        }
                        if let Some(desc) = &user.description {
                            if !desc.is_empty() { info_row(ui, "Role", desc); }
                        }
                        if let Some(y) = self.student_year {
                            info_row(ui, "Year", year_label(y));
                        }
                    });

                ui.add_space(12.0);

                if user.phone1.is_some() || user.city.is_some() {
                    egui::Frame::none()
                        .fill(ui.visuals().faint_bg_color)
                        .rounding(8.0)
                        .inner_margin(egui::Margin::symmetric(20.0, 16.0))
                        .show(ui, |ui| {
                            ui.set_min_width(card_w);
                            ui.label(RichText::new("Contact").size(14.0).strong());
                            ui.separator();
                            ui.add_space(6.0);
                            if let Some(p) = &user.phone1 { info_row(ui, "Phone", p); }
                            if let Some(p) = &user.phone2 { info_row(ui, "Mobile", p); }
                            if let Some(c) = &user.city  { info_row(ui, "City", c); }
                            if let Some(c) = &user.country { info_row(ui, "Country", c); }
                        });
                    ui.add_space(12.0);
                }

                if !self.dept_counts.is_empty() {
                    egui::Frame::none()
                        .fill(ui.visuals().faint_bg_color)
                        .rounding(8.0)
                        .inner_margin(egui::Margin::symmetric(20.0, 16.0))
                        .show(ui, |ui| {
                            ui.set_min_width(card_w);
                            ui.label(RichText::new("Enrolled Courses by Department").size(14.0).strong());
                            ui.separator();
                            ui.add_space(6.0);
                            for (dept, count) in &self.dept_counts {
                                info_row(ui, dept, &format!("{count} course{}", if *count == 1 { "" } else { "s" }));
                            }
                        });
                }
            });

            ui.add_space(24.0);
        });
    }
}

use egui::{Align, Layout, Ui};

pub struct LoginScreen {
    pub username: String,
    pub password: String,
    pub error: Option<String>,
    pub loading: bool,
}

impl Default for LoginScreen {
    fn default() -> Self {
        Self {
            username: String::new(),
            password: String::new(),
            error: None,
            loading: false,
        }
    }
}

impl LoginScreen {
    pub fn show(&mut self, ui: &mut Ui) -> bool {
        let mut submit = false;

        // Centre vertically
        let available = ui.available_height();
        ui.add_space((available * 0.5 - 140.0).max(20.0));

        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new("Campus LMS").size(26.0).strong());
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("University of Colombo — Faculty of Science")
                    .size(12.0)
                    .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(28.0);

            // Card frame
            egui::Frame::none()
                .fill(ui.visuals().extreme_bg_color)
                .rounding(8.0)
                .inner_margin(egui::Margin::symmetric(28.0, 24.0))
                .shadow(egui::epaint::Shadow {
                    offset: egui::Vec2::new(0.0, 2.0),
                    blur: 12.0,
                    spread: 0.0,
                    color: egui::Color32::from_black_alpha(60),
                })
                .show(ui, |ui| {
                    ui.set_width(320.0);

                    ui.label(egui::RichText::new("Email").size(12.0));
                    ui.add_space(3.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.username)
                            .desired_width(f32::INFINITY)
                            .hint_text("xxxxx@stu.cmb.ac.lk"),
                    );
                    ui.add_space(12.0);

                    ui.label(egui::RichText::new("Password").size(12.0));
                    ui.add_space(3.0);
                    let pass_resp = ui.add(
                        egui::TextEdit::singleline(&mut self.password)
                            .desired_width(f32::INFINITY)
                            .password(true),
                    );
                    // Submit on Enter
                    if pass_resp.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        submit = true;
                    }

                    ui.add_space(18.0);

                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        if self.loading {
                            ui.spinner();
                        } else {
                            let btn = ui.add_sized(
                                [f32::INFINITY, 32.0],
                                egui::Button::new(
                                    egui::RichText::new("Login").size(14.0),
                                ),
                            );
                            if btn.clicked() {
                                submit = true;
                            }
                        }
                    });

                    if let Some(err) = &self.error {
                        ui.add_space(10.0);
                        ui.centered_and_justified(|ui| {
                            ui.colored_label(egui::Color32::from_rgb(255, 100, 100), err);
                        });
                    }
                });
        });

        submit
    }
}

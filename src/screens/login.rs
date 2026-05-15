use egui::Ui;

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

        let top_pad = (ui.available_height() * 0.22).max(20.0);
        ui.add_space(top_pad);

        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new("Campus LMS").size(28.0).strong());
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("University of Colombo — Faculty of Science")
                    .size(12.0)
                    .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(22.0);

            egui::Frame::none()
                .fill(ui.visuals().window_fill())
                .stroke(egui::Stroke::new(
                    1.0,
                    ui.visuals().widgets.noninteractive.bg_stroke.color,
                ))
                .rounding(10.0)
                .inner_margin(egui::Margin::symmetric(28.0, 24.0))
                .show(ui, |ui| {
                    ui.set_min_width(300.0);
                    ui.set_max_width(300.0);

                    ui.label(egui::RichText::new("Email").size(12.0).strong());
                    ui.add_space(4.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.username)
                            .desired_width(f32::INFINITY)
                            .hint_text("xxxxx@stu.cmb.ac.lk"),
                    );
                    ui.add_space(16.0);

                    ui.label(egui::RichText::new("Password").size(12.0).strong());
                    ui.add_space(4.0);
                    let pass_resp = ui.add(
                        egui::TextEdit::singleline(&mut self.password)
                            .desired_width(f32::INFINITY)
                            .password(true),
                    );
                    if pass_resp.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        submit = true;
                    }

                    ui.add_space(24.0);

                    if self.loading {
                        ui.vertical_centered(|ui| {
                            ui.spinner();
                        });
                    } else {
                        let btn = ui.add_sized(
                            [ui.available_width(), 32.0],
                            egui::Button::new(
                                egui::RichText::new("Login").size(14.0).strong(),
                            ),
                        );
                        if btn.clicked() {
                            submit = true;
                        }
                    }

                    if let Some(err) = &self.error {
                        ui.add_space(12.0);
                        ui.vertical_centered(|ui| {
                            ui.colored_label(egui::Color32::from_rgb(255, 100, 100), err);
                        });
                    }
                });
        });

        submit
    }
}

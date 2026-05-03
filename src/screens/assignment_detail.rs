use egui::Ui;
use crate::api::types::{Assignment, SubmissionStatusResponse, SubmissionFile};
use crate::models::decode_html;

pub enum AssignmentDetailEvent {
    Back,
    UploadFile,
    SubmitForGrading,
    OpenFile { url: String },
    OpenInBrowser { url: String },
}

#[derive(Default, PartialEq)]
pub enum UploadState {
    #[default]
    Idle,
    Uploading,
    Done,
    Error(String),
}

pub struct AssignmentDetailScreen {
    pub assignment: Option<Assignment>,
    pub course_name: String,
    pub status: Option<SubmissionStatusResponse>,
    pub loading_status: bool,
    pub upload_state: UploadState,
    pub pending_file: Option<(String, Vec<u8>)>, // (filename, bytes) chosen but not yet uploaded
}

impl Default for AssignmentDetailScreen {
    fn default() -> Self {
        Self {
            assignment: None,
            course_name: String::new(),
            status: None,
            loading_status: false,
            upload_state: UploadState::Idle,
            pending_file: None,
        }
    }
}

fn fmt_date(ts: i64) -> String {
    if ts == 0 { return "No due date".into(); }
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.with_timezone(&chrono::Local).format("%a, %d %b %Y  %H:%M").to_string())
        .unwrap_or_default()
}

fn fmt_size(b: u64) -> String {
    if b >= 1_048_576 { format!("{:.1} MB", b as f64 / 1_048_576.0) }
    else if b >= 1024  { format!("{} KB", b / 1024) }
    else               { format!("{} B", b) }
}

fn strip_html(s: &str) -> String {
    s.chars().fold((String::new(), false), |(mut acc, in_tag), c| {
        if c == '<' { (acc, true) }
        else if c == '>' { (acc, false) }
        else if !in_tag { acc.push(c); (acc, false) }
        else { (acc, in_tag) }
    }).0
}

impl AssignmentDetailScreen {
    pub fn show(&mut self, ui: &mut Ui) -> Option<AssignmentDetailEvent> {
        let Some(assign) = &self.assignment else {
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new("Select an assignment")
                    .color(ui.visuals().weak_text_color()));
            });
            return None;
        };
        let assign = assign.clone();
        let mut event = None;

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(8.0);

            // ── Back + Header ────────────────────────────────────────────────
            ui.horizontal(|ui| {
                if ui.button(egui::RichText::new(format!("{} Back", egui_phosphor::regular::ARROW_LEFT)).size(13.0)).clicked() {
                    event = Some(AssignmentDetailEvent::Back);
                }
            });
            ui.add_space(6.0);
            ui.label(egui::RichText::new(decode_html(&assign.name)).size(20.0).strong());
            ui.add_space(2.0);
            ui.label(egui::RichText::new(&self.course_name)
                .size(12.0).color(ui.visuals().weak_text_color()));
            ui.add_space(8.0);

            // Due date
            let now = chrono::Utc::now().timestamp();
            if assign.duedate > 0 {
                let overdue = assign.duedate < now;
                let color = if overdue {
                    egui::Color32::from_rgb(220, 80, 60)
                } else {
                    egui::Color32::from_rgb(100, 200, 120)
                };
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(egui_phosphor::regular::CLOCK)
                        .color(color).size(13.0));
                    ui.label(egui::RichText::new(
                        format!("Due: {}{}",
                            fmt_date(assign.duedate),
                            if overdue { "  (OVERDUE)" } else { "" }))
                        .size(13.0).color(color));
                });
            }
            if assign.cutoffdate > 0 && assign.cutoffdate != assign.duedate {
                ui.label(egui::RichText::new(format!("Cut-off: {}", fmt_date(assign.cutoffdate)))
                    .size(11.0).color(ui.visuals().weak_text_color()));
            }
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(8.0);

            // ── Description ──────────────────────────────────────────────────
            if let Some(intro) = &assign.intro {
                let text = strip_html(&decode_html(intro));
                if !text.trim().is_empty() {
                    ui.label(egui::RichText::new("Instructions").size(13.0).strong());
                    ui.add_space(4.0);
                    egui::Frame::none()
                        .fill(ui.visuals().faint_bg_color)
                        .rounding(6.0)
                        .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.label(egui::RichText::new(text.trim()).size(13.0));
                        });
                    ui.add_space(10.0);
                }
            }

            // ── Submission status ─────────────────────────────────────────────
            ui.label(egui::RichText::new("Submission Status").size(13.0).strong());
            ui.add_space(4.0);

            if self.loading_status {
                ui.horizontal(|ui| { ui.spinner(); ui.label("Loading..."); });
            } else if let Some(status) = &self.status {
                let attempt = status.lastattempt.as_ref();
                let submission = attempt.and_then(|a| a.submission.as_ref());
                let sub_status = submission.map(|s| s.status.as_str()).unwrap_or("new");
                let grading_status = attempt.map(|a| a.gradingstatus.as_str()).unwrap_or("notgraded");
                let can_edit = attempt.map(|a| a.canedit).unwrap_or(true);

                egui::Frame::none()
                    .fill(ui.visuals().faint_bg_color)
                    .rounding(6.0)
                    .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());

                        let (status_color, status_text) = match sub_status {
                            "submitted" => (egui::Color32::from_rgb(60, 200, 100), "Submitted for grading"),
                            "draft"     => (egui::Color32::from_rgb(255, 180, 50), "Draft (not submitted)"),
                            _           => (egui::Color32::from_rgb(180, 180, 180), "Not submitted"),
                        };
                        status_row(ui, "Submission", status_text, status_color);

                        let (g_color, g_text) = match grading_status {
                            "graded"    => (egui::Color32::from_rgb(60, 200, 100), "Graded"),
                            "notgraded" => (egui::Color32::from_rgb(180, 180, 180), "Not graded"),
                            other       => (egui::Color32::from_rgb(180, 180, 180), other),
                        };
                        status_row(ui, "Grading", g_text, g_color);

                        if let Some(sub) = submission {
                            if sub.timemodified > 0 {
                                status_row(ui, "Last modified",
                                    &fmt_date(sub.timemodified),
                                    ui.visuals().text_color());
                            }

                            // Submitted files
                            let files: Vec<&SubmissionFile> = sub.plugins.iter()
                                .flat_map(|p| p.fileareas.iter())
                                .filter(|fa| fa.area == "submission_files")
                                .flat_map(|fa| fa.files.iter())
                                .filter(|f| !f.filename.is_empty())
                                .collect();

                            if !files.is_empty() {
                                ui.add_space(6.0);
                                ui.label(egui::RichText::new("Submitted files")
                                    .size(12.0).strong());
                                ui.add_space(2.0);
                                for file in files {
                                    let file = file.clone();
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(egui_phosphor::regular::FILE_TEXT)
                                            .size(12.0).color(ui.visuals().weak_text_color()));
                                        let resp = ui.selectable_label(false,
                                            egui::RichText::new(&file.filename)
                                                .size(12.0)
                                                .color(egui::Color32::from_rgb(100, 160, 230)));
                                        if resp.clicked() {
                                            event = Some(AssignmentDetailEvent::OpenFile {
                                                url: file.fileurl.clone(),
                                            });
                                        }
                                        ui.label(egui::RichText::new(fmt_size(file.filesize))
                                            .size(10.0).color(ui.visuals().weak_text_color()));
                                    });
                                }
                            }
                        }
                    });

                ui.add_space(10.0);

                // ── Upload + Submit buttons ───────────────────────────────────
                if can_edit {
                    ui.horizontal(|ui| {
                        // Pending file preview
                        if let Some((fname, _)) = &self.pending_file {
                            egui::Frame::none()
                                .fill(egui::Color32::from_rgba_premultiplied(0, 60, 100, 80))
                                .rounding(4.0)
                                .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                                .show(ui, |ui| {
                                    ui.label(egui::RichText::new(
                                        format!("{} {fname}", egui_phosphor::regular::PAPERCLIP))
                                        .size(12.0));
                                });
                            ui.add_space(4.0);
                        }

                        let uploading = self.upload_state == UploadState::Uploading;

                        let pick_btn = ui.add_enabled(
                            !uploading,
                            egui::Button::new(egui::RichText::new(
                                format!("{} Choose file", egui_phosphor::regular::UPLOAD_SIMPLE))
                                .size(13.0)));
                        if pick_btn.clicked() {
                            event = Some(AssignmentDetailEvent::UploadFile);
                        }

                        if self.pending_file.is_some() {
                            let submit_btn = ui.add_enabled(
                                !uploading,
                                egui::Button::new(egui::RichText::new(
                                    format!("{} Upload & Submit", egui_phosphor::regular::PAPER_PLANE_TILT))
                                    .size(13.0))
                                    .fill(egui::Color32::from_rgb(40, 120, 60)));
                            if submit_btn.clicked() {
                                event = Some(AssignmentDetailEvent::SubmitForGrading);
                            }
                        }

                        if uploading {
                            ui.spinner();
                            ui.label(egui::RichText::new("Uploading…")
                                .size(12.0).color(ui.visuals().weak_text_color()));
                        }
                    });

                    if let UploadState::Error(e) = &self.upload_state {
                        ui.label(egui::RichText::new(format!("Upload failed: {e}"))
                            .size(11.0).color(egui::Color32::from_rgb(220, 80, 80)));
                    }
                    if self.upload_state == UploadState::Done {
                        ui.label(egui::RichText::new(
                            format!("{} Submitted successfully!", egui_phosphor::regular::CHECK_CIRCLE))
                            .size(13.0).color(egui::Color32::from_rgb(60, 200, 100)));
                    }
                }
            }

            ui.add_space(12.0);

            // Open in browser fallback
            if let Some(url) = assign.url() {
                if ui.small_button("Open in browser").clicked() {
                    event = Some(AssignmentDetailEvent::OpenInBrowser { url });
                }
            }
        });

        event
    }
}

fn status_row(ui: &mut Ui, label: &str, value: &str, color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{label}:"))
            .size(12.0).color(ui.visuals().weak_text_color()));
        ui.label(egui::RichText::new(value).size(12.0).color(color));
    });
}

use std::path::PathBuf;
use std::collections::HashMap;
use egui::Ui;
use chrono::{Datelike, NaiveDate, Utc};
use crate::api::types::{CourseSection, CourseModule};
use crate::models::decode_html;

const MOODLE_BASE: &str = "https://sci.cmb.ac.lk/lms";

pub enum CourseContentEvent {
    OpenUrl(String),
    Download { module_id: u64, url: String, save_path: PathBuf },
    OpenFile(PathBuf),
    ShowFolder(PathBuf),
}

#[derive(Clone)]
pub enum DownloadState {
    Downloading,
    Done(PathBuf),
    Error(String),
}

pub struct CourseContentScreen {
    pub course_id: u64,
    pub course_shortname: String,
    pub sections: Vec<CourseSection>,
    pub loading: bool,
    pub needs_scroll: bool,
    pub download_states: HashMap<u64, DownloadState>,
}

impl Default for CourseContentScreen {
    fn default() -> Self {
        Self {
            course_id: 0,
            course_shortname: String::new(),
            sections: vec![],
            loading: false,
            needs_scroll: true,
            download_states: HashMap::new(),
        }
    }
}

/// Returns the organized download base directory: ~/Downloads/Campus LMS/
pub fn download_base() -> PathBuf {
    dirs_next::download_dir()
        .unwrap_or_else(|| dirs_next::home_dir().unwrap_or_default())
        .join("Campus LMS")
}

fn sanitize(s: &str) -> String {
    s.chars().map(|c| match c {
        '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
        c => c,
    }).collect::<String>().trim().to_string()
}

/// Build the full save path for a file.
pub fn file_save_path(course: &str, section: &str, filename: &str) -> PathBuf {
    download_base()
        .join(sanitize(course))
        .join(sanitize(section))
        .join(sanitize(filename))
}

fn is_video(filename: &str) -> bool {
    matches!(
        filename.rsplit('.').next().map(|e| e.to_lowercase()).as_deref(),
        Some("mp4" | "webm" | "avi" | "mov" | "mkv" | "flv" | "wmv" | "m4v" | "ogv")
    )
}

fn is_audio(filename: &str) -> bool {
    matches!(
        filename.rsplit('.').next().map(|e| e.to_lowercase()).as_deref(),
        Some("mp3" | "wav" | "ogg" | "flac" | "aac" | "m4a" | "opus")
    )
}

fn is_streamable(filename: &str) -> bool {
    is_video(filename) || is_audio(filename)
}

fn fmt_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{} B", bytes)
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

fn parse_section_date(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    let months = ["jan","feb","mar","apr","may","jun","jul","aug","sep","oct","nov","dec"];
    let parts: Vec<&str> = s.split_whitespace().collect();
    let (day_str, month_str, year_opt) = match parts.len() {
        2 => (parts[0], parts[1], None),
        3 => (parts[0], parts[1], parts[2].parse::<i32>().ok()),
        _ => return None,
    };
    let day = day_str.parse::<u32>().ok()?;
    let month_idx = months.iter().position(|&m| m == month_str.to_lowercase().as_str())? as u32 + 1;
    let year = year_opt.unwrap_or_else(|| Utc::now().date_naive().year());
    NaiveDate::from_ymd_opt(year, month_idx, day)
}

fn section_is_current_week(name: &str) -> bool {
    let today = Utc::now().date_naive();
    let parts: Vec<&str> = name.splitn(2, " - ").collect();
    if parts.len() != 2 { return false; }
    let left = {
        let s = parts[0].trim();
        match s.find(|c: char| c.is_ascii_digit()) {
            Some(i) => &s[i..],
            None => return false,
        }
    };
    let right = parts[1].trim().trim_end_matches(')');
    let start = match parse_section_date(left) { Some(d) => d, None => return false };
    let end   = parse_section_date(right).unwrap_or(start + chrono::Duration::days(6));
    today >= start && today <= end
}

impl CourseContentScreen {
    pub fn show(&mut self, ui: &mut Ui) -> Option<CourseContentEvent> {
        let mut event = None;

        if self.loading && self.sections.is_empty() {
            ui.centered_and_justified(|ui| { ui.spinner(); });
            return None;
        }

        if self.sections.is_empty() {
            ui.add_space(20.0);
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new("No content found or still loading...")
                    .color(ui.visuals().weak_text_color()));
            });
            return None;
        }

        let current_week_idx: Option<usize> = self.sections.iter().enumerate()
            .find(|(_, s)| section_is_current_week(&s.name))
            .map(|(i, _)| i);

        let scroll_id = egui::Id::new(("course_scroll", self.course_id));
        let mut scroll = egui::ScrollArea::vertical().auto_shrink([false, false]).id_salt(scroll_id);

        if self.needs_scroll {
            if let Some(idx) = current_week_idx {
                let approx_offset: f32 = self.sections.iter().take(idx)
                    .filter(|s| !s.modules.is_empty() || !s.summary.is_empty())
                    .map(|s| 80.0 + s.modules.len() as f32 * 58.0)
                    .sum();
                scroll = scroll.vertical_scroll_offset(approx_offset);
            }
            self.needs_scroll = false;
        }

        let course_shortname = self.course_shortname.clone();

        scroll.show(ui, |ui| {
            ui.add_space(8.0);
            for section in &self.sections {
                if section.modules.is_empty() && section.summary.is_empty() { continue; }

                let is_current = section_is_current_week(&section.name);
                let section_name = section.name.clone();

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&section.name).size(16.0).strong());
                    if is_current {
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgb(40, 100, 40))
                            .rounding(4.0)
                            .inner_margin(egui::Margin::symmetric(6.0, 2.0))
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new("This week")
                                    .size(10.0).strong()
                                    .color(egui::Color32::from_rgb(100, 220, 100)));
                            });
                    }
                });

                let summary = decode_html(&section.summary);
                if !summary.trim().is_empty() {
                    ui.label(egui::RichText::new(summary.trim()).size(12.0)
                        .color(ui.visuals().weak_text_color()));
                }

                if is_current {
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 2.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgb(60, 160, 60));
                } else {
                    ui.separator();
                }
                ui.add_space(4.0);

                for module in &section.modules {
                    let ds = self.download_states.get(&module.id).cloned();
                    if let Some(ev) = render_module(ui, module, &course_shortname, &section_name, ds) {
                        event = Some(ev);
                    }
                }
            }
            ui.add_space(20.0);
        });

        event
    }
}

fn render_module(
    ui: &mut Ui,
    module: &CourseModule,
    course: &str,
    section: &str,
    download_state: Option<DownloadState>,
) -> Option<CourseContentEvent> {
    let mut event = None;

    let (icon, tooltip) = match module.modname.as_str() {
        "assign"   => (egui_phosphor::regular::PENCIL_SIMPLE, "Assignment"),
        "resource" => (egui_phosphor::regular::FILE_TEXT, "File Resource"),
        "forum"    => (egui_phosphor::regular::CHATS, "Forum"),
        "quiz"     => (egui_phosphor::regular::QUESTION, "Quiz"),
        "folder"   => (egui_phosphor::regular::FOLDER, "Folder"),
        "url"      => (egui_phosphor::regular::LINK, "External Link"),
        "label"    => ("", ""),
        _          => (egui_phosphor::regular::PACKAGE, "Module Item"),
    };

    if module.modname == "label" {
        let text = decode_html(&module.name);
        if !text.trim().is_empty() {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(text.trim()).size(13.0));
            ui.add_space(4.0);
        }
        return None;
    }

    // "page" module: render inline if content available
    if module.modname == "page" {
        if let Some(html) = &module.mainpage {
            let text = strip_html(&decode_html(html));
            if !text.trim().is_empty() {
                egui::Frame::none()
                    .fill(ui.visuals().faint_bg_color)
                    .rounding(6.0)
                    .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(egui::RichText::new(decode_html(&module.name))
                            .size(14.0).strong());
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(text.trim()).size(12.0));
                    });
                ui.add_space(4.0);
                return None;
            }
        }
    }

    // Determine file info and URLs
    let first_file = module.contents.first();
    let fileurl = first_file.map(|f| f.fileurl.clone());
    let filename = first_file.map(|f| f.filename.as_str()).unwrap_or("");
    let filesize = first_file.map(|f| f.filesize).unwrap_or(0);

    // Choose action URL based on module type
    let is_file_module = matches!(module.modname.as_str(), "resource" | "folder");
    let is_media = is_streamable(filename);

    let action: ModuleAction = if is_file_module {
        if let Some(url) = &fileurl {
            if is_media {
                ModuleAction::Stream(url.clone(), filename.to_string())
            } else {
                // Check if already downloaded
                let save_path = file_save_path(course, section, filename);
                match &download_state {
                    Some(DownloadState::Done(p)) if p.exists() => {
                        ModuleAction::AlreadyDownloaded(p.clone())
                    }
                    _ => ModuleAction::Download(url.clone(), filename.to_string(), save_path),
                }
            }
        } else {
            ModuleAction::None
        }
    } else if module.modname == "url" {
        module.url.as_ref().map(|u| ModuleAction::OpenUrl(u.clone()))
            .unwrap_or(ModuleAction::None)
    } else {
        // assign, quiz, forum, page (fallback), etc. — open via wantsurl
        let url = module.url.clone()
            .or_else(|| fileurl.clone())
            .unwrap_or_else(|| format!("{MOODLE_BASE}/mod/{}/view.php?id={}", module.modname, module.id));
        ModuleAction::OpenUrl(url)
    };

    // Card
    let inner = egui::Frame::none()
        .fill(match &action {
            ModuleAction::AlreadyDownloaded(_) =>
                egui::Color32::from_rgba_premultiplied(20, 60, 20, 40),
            _ => ui.visuals().faint_bg_color,
        })
        .rounding(6.0)
        .inner_margin(egui::Margin::symmetric(10.0, 8.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                if !icon.is_empty() {
                    ui.label(egui::RichText::new(icon).size(14.0)
                        .color(ui.visuals().weak_text_color()))
                        .on_hover_text(tooltip);
                    ui.add_space(4.0);
                }
                ui.vertical(|ui| {
                    let name_color = match &action {
                        ModuleAction::None => ui.visuals().text_color(),
                        ModuleAction::AlreadyDownloaded(_) =>
                            egui::Color32::from_rgb(80, 200, 80),
                        _ => egui::Color32::from_rgb(100, 160, 230),
                    };
                    ui.label(egui::RichText::new(decode_html(&module.name))
                        .size(14.0).color(name_color));

                    // Sub-label
                    match &action {
                        ModuleAction::Download(_, fname, _) => {
                            let ext = fname.rsplit('.').next().unwrap_or("").to_uppercase();
                            ui.label(egui::RichText::new(
                                format!("{ext}  ·  {}  — click to download", fmt_size(filesize)))
                                .size(10.5).color(ui.visuals().weak_text_color()));
                        }
                        ModuleAction::Stream(_, fname) => {
                            let ext = fname.rsplit('.').next().unwrap_or("").to_uppercase();
                            ui.label(egui::RichText::new(
                                format!("{ext}  ·  {}  — click to stream", fmt_size(filesize)))
                                .size(10.5).color(egui::Color32::from_rgb(100, 180, 255)));
                        }
                        ModuleAction::AlreadyDownloaded(path) => {
                            let name = path.file_name()
                                .and_then(|n| n.to_str()).unwrap_or("");
                            ui.label(egui::RichText::new(format!("Downloaded: {name}"))
                                .size(10.5).color(egui::Color32::from_rgb(80, 200, 80)));
                        }
                        _ => {
                            if let Some(desc) = &module.description {
                                let d = decode_html(desc);
                                if !d.trim().is_empty() {
                                    ui.label(egui::RichText::new(d.trim()).size(11.0)
                                        .color(ui.visuals().weak_text_color()));
                                }
                            }
                        }
                    }

                    // Downloading spinner
                    if let Some(DownloadState::Downloading) = &download_state {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(egui::RichText::new("Downloading…").size(10.5)
                                .color(ui.visuals().weak_text_color()));
                        });
                    }
                    if let Some(DownloadState::Error(e)) = &download_state {
                        ui.label(egui::RichText::new(format!("Error: {e}"))
                            .size(10.5).color(egui::Color32::from_rgb(220, 80, 80)));
                    }

                    // "Show in folder" button for downloaded files
                    if let ModuleAction::AlreadyDownloaded(path) = &action {
                        if ui.small_button("Show in folder").clicked() {
                            event = Some(CourseContentEvent::ShowFolder(path.clone()));
                        }
                    }
                });
            });
        });

    // Click on card
    let has_action = !matches!(&action, ModuleAction::None);
    let is_downloading = matches!(&download_state, Some(DownloadState::Downloading));

    if has_action && !is_downloading {
        let click_resp = ui.interact(
            inner.response.rect,
            egui::Id::new(("module_click", module.id)),
            egui::Sense::click(),
        );
        if click_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if click_resp.clicked() {
            event = Some(match action {
                ModuleAction::OpenUrl(url) => CourseContentEvent::OpenUrl(url),
                ModuleAction::Stream(url, _) => CourseContentEvent::OpenUrl(url), // system player streams
                ModuleAction::Download(url, _, path) => CourseContentEvent::Download {
                    module_id: module.id, url, save_path: path,
                },
                ModuleAction::AlreadyDownloaded(path) => CourseContentEvent::OpenFile(path),
                ModuleAction::None => unreachable!(),
            });
        }
    }

    ui.add_space(4.0);
    event
}

enum ModuleAction {
    OpenUrl(String),
    Stream(String, String),            // url, filename
    Download(String, String, PathBuf), // url, filename, save_path
    AlreadyDownloaded(PathBuf),
    None,
}

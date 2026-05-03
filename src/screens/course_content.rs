use egui::Ui;
use crate::api::types::{CourseSection, CourseModule};
use crate::models::decode_html;

pub struct CourseContentScreen {
    pub course_id: u64,
    pub sections: Vec<CourseSection>,
    pub loading: bool,
}

impl Default for CourseContentScreen {
    fn default() -> Self {
        Self {
            course_id: 0,
            sections: vec![],
            loading: false,
        }
    }
}

impl CourseContentScreen {
    pub fn show(&mut self, ui: &mut Ui) -> Option<String> {
        let mut open_url = None;

        if self.loading {
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

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(8.0);
            for section in &self.sections {
                if section.modules.is_empty() && section.summary.is_empty() { continue; }
                
                ui.add_space(12.0);
                ui.label(egui::RichText::new(&section.name).size(16.0).strong());
                
                let summary = decode_html(&section.summary);
                if !summary.trim().is_empty() {
                    ui.label(egui::RichText::new(summary.trim()).size(12.0).color(ui.visuals().weak_text_color()));
                }
                ui.separator();
                ui.add_space(4.0);
                
                for module in &section.modules {
                    if let Some(url) = render_module(ui, module) {
                        open_url = Some(url);
                    }
                }
            }
            ui.add_space(20.0);
        });

        open_url
    }
}

fn render_module(ui: &mut Ui, module: &CourseModule) -> Option<String> {
    let mut url_to_open = None;

    let (icon, tooltip) = match module.modname.as_str() {
        "assign" => (egui_phosphor::regular::PENCIL_SIMPLE, "Assignment"),
        "resource" => (egui_phosphor::regular::FILE_TEXT, "File Resource"),
        "forum" => (egui_phosphor::regular::CHATS, "Forum"),
        "quiz" => (egui_phosphor::regular::QUESTION, "Quiz"),
        "folder" => (egui_phosphor::regular::FOLDER, "Folder"),
        "url" => (egui_phosphor::regular::LINK, "External Link"),
        "label" => ("", ""),
        _ => (egui_phosphor::regular::PACKAGE, "Module Item"),
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

    let resp = egui::Frame::none()
        .fill(ui.visuals().faint_bg_color)
        .rounding(6.0)
        .inner_margin(egui::Margin::symmetric(10.0, 8.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                if !icon.is_empty() {
                    ui.label(egui::RichText::new(icon).size(14.0).color(ui.visuals().weak_text_color()))
                        .on_hover_text(tooltip);
                    ui.add_space(4.0);
                }
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(decode_html(&module.name)).size(14.0));
                    if let Some(desc) = &module.description {
                        let d = decode_html(desc);
                        if !d.trim().is_empty() {
                            ui.label(egui::RichText::new(d.trim()).size(11.0).color(ui.visuals().weak_text_color()));
                        }
                    }
                });
            });
        }).response;
    
    if resp.interact(egui::Sense::click()).clicked() {
        if let Some(url) = &module.url {
            url_to_open = Some(url.clone());
        } else if !module.contents.is_empty() {
            if let Some(first) = module.contents.first() {
                url_to_open = Some(first.fileurl.clone());
            }
        }
    }
    
    ui.add_space(4.0);
    url_to_open
}

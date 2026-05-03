use similar::{ChangeTag, TextDiff};
use crate::api::types::CourseSection;
use crate::storage::{ContentChange, StoredFingerprint};

pub fn fmt_bytes(b: u64) -> String {
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

/// Compute a unified-style text diff between two plain-text strings.
/// Returns lines prefixed with '+', '-', or ' ' (context).
pub fn text_diff(old: &str, new: &str) -> String {
    if old == new { return String::new(); }
    let diff = TextDiff::from_lines(old, new);
    let mut out = String::new();
    for change in diff.iter_all_changes() {
        let prefix = match change.tag() {
            ChangeTag::Delete  => "- ",
            ChangeTag::Insert  => "+ ",
            ChangeTag::Equal   => "  ",
        };
        // Skip long equal runs — only show context lines adjacent to changes
        if change.tag() == ChangeTag::Equal {
            continue; // omit context lines for compactness
        }
        out.push_str(prefix);
        out.push_str(change.value().trim_end_matches('\n'));
        out.push('\n');
    }
    out.trim_end().to_string()
}

/// Module description text used for diffing (strips HTML, normalises whitespace).
pub fn module_text(module: &crate::api::types::CourseModule) -> String {
    let raw = module.mainpage.as_deref()
        .or(module.description.as_deref())
        .unwrap_or("");
    let stripped = strip_html(raw);
    // Collapse repeated whitespace to single spaces / newlines
    stripped.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Diff new course sections against stored fingerprints.
/// Returns (changes, new_fps_to_upsert, removed_module_ids).
/// On first run (stored is empty), only saves fingerprints without generating changes.
pub fn diff_content(
    course_id: u64,
    sections: &[CourseSection],
    stored: &[StoredFingerprint],
) -> (Vec<ContentChange>, Vec<StoredFingerprint>, Vec<u64>) {
    let stored_map: std::collections::HashMap<u64, &StoredFingerprint> =
        stored.iter().map(|fp| (fp.module_id, fp)).collect();
    let first_run = stored.is_empty();

    let mut new_fps: Vec<StoredFingerprint> = vec![];
    let mut changes: Vec<ContentChange> = vec![];
    let mut seen_ids: std::collections::HashSet<u64> = Default::default();

    for section in sections {
        for module in &section.modules {
            seen_ids.insert(module.id);
            let filesize = module.contents.first().map(|f| f.filesize as i64).unwrap_or(0);
            let fileurl  = module.contents.first().map(|f| f.fileurl.as_str()).unwrap_or("").to_string();
            let desc_text = module_text(module);

            let new_fp = StoredFingerprint {
                module_id: module.id,
                name: module.name.clone(),
                filesize,
                fileurl: fileurl.clone(),
                description: desc_text.clone(),
            };

            if let Some(old) = stored_map.get(&module.id) {
                if old.name != module.name {
                    changes.push(ContentChange {
                        course_id, module_id: module.id,
                        module_name: module.name.clone(),
                        section_name: section.name.clone(),
                        change_type: "renamed".into(),
                        old_val: old.name.clone(),
                        new_val: module.name.clone(),
                        detected_at: 0,
                    });
                }

                if old.filesize != filesize || (!fileurl.is_empty() && old.fileurl != fileurl) {
                    changes.push(ContentChange {
                        course_id, module_id: module.id,
                        module_name: module.name.clone(),
                        section_name: section.name.clone(),
                        change_type: "file_updated".into(),
                        old_val: fmt_bytes(old.filesize as u64),
                        new_val: fmt_bytes(filesize as u64),
                        detected_at: 0,
                    });
                }

                // Text diff for description / page content
                if !old.description.is_empty() && old.description != desc_text && !desc_text.is_empty() {
                    let diff = text_diff(&old.description, &desc_text);
                    if !diff.is_empty() {
                        changes.push(ContentChange {
                            course_id, module_id: module.id,
                            module_name: module.name.clone(),
                            section_name: section.name.clone(),
                            change_type: "description_updated".into(),
                            old_val: old.description.clone(),
                            new_val: diff, // store the computed diff, not the new text
                            detected_at: 0,
                        });
                    }
                }
            } else if !first_run {
                changes.push(ContentChange {
                    course_id, module_id: module.id,
                    module_name: module.name.clone(),
                    section_name: section.name.clone(),
                    change_type: "added".into(),
                    old_val: String::new(),
                    new_val: module.name.clone(),
                    detected_at: 0,
                });
            }

            new_fps.push(new_fp);
        }
    }

    // Detect removed modules
    let removed_ids: Vec<u64> = stored.iter()
        .filter(|fp| !seen_ids.contains(&fp.module_id))
        .map(|fp| fp.module_id)
        .collect();

    if !first_run {
        for &rid in &removed_ids {
            if let Some(old) = stored_map.get(&rid) {
                changes.push(ContentChange {
                    course_id, module_id: rid,
                    module_name: old.name.clone(),
                    section_name: String::new(),
                    change_type: "removed".into(),
                    old_val: old.name.clone(),
                    new_val: String::new(),
                    detected_at: 0,
                });
            }
        }
    }

    (changes, new_fps, removed_ids)
}

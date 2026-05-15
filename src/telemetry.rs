use similar::{ChangeTag, TextDiff};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use crate::api::types::CourseSection;
use crate::storage::{ContentChange, StoredFingerprint, StoredSectionFingerprint};

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

pub fn compute_module_hash(module: &crate::api::types::CourseModule) -> String {
    let mut s = DefaultHasher::new();
    module.name.hash(&mut s);
    module.description.hash(&mut s);
    module.mainpage.hash(&mut s);
    for c in &module.contents {
        c.filename.hash(&mut s);
        c.filesize.hash(&mut s);
        c.timemodified.hash(&mut s);
        c.fileurl.hash(&mut s);
    }
    for d in &module.dates {
        d.label.hash(&mut s);
        d.timestamp.hash(&mut s);
    }
    format!("{:x}", s.finish())
}

pub fn compute_section_hash(section: &CourseSection) -> String {
    let mut s = DefaultHasher::new();
    section.name.hash(&mut s);
    section.summary.hash(&mut s);
    format!("{:x}", s.finish())
}

/// Diff new course sections against stored fingerprints.
/// Returns (changes, new_module_fps, new_section_fps, removed_module_ids, removed_section_ids).
pub fn diff_content(
    course_id: u64,
    sections: &[CourseSection],
    stored_modules: &[StoredFingerprint],
    stored_sections: &[StoredSectionFingerprint],
) -> (
    Vec<ContentChange>,
    Vec<StoredFingerprint>,
    Vec<StoredSectionFingerprint>,
    Vec<u64>,
    Vec<u64>,
) {
    let mod_map: std::collections::HashMap<u64, &StoredFingerprint> =
        stored_modules.iter().map(|fp| (fp.module_id, fp)).collect();
    let sec_map: std::collections::HashMap<u64, &StoredSectionFingerprint> =
        stored_sections.iter().map(|fp| (fp.section_id, fp)).collect();
    
    let first_run = stored_modules.is_empty() && stored_sections.is_empty();

    let mut new_mod_fps = vec![];
    let mut new_sec_fps = vec![];
    let mut changes = vec![];
    let mut seen_mod_ids = std::collections::HashSet::new();
    let mut seen_sec_ids = std::collections::HashSet::new();

    for section in sections {
        seen_sec_ids.insert(section.id);
        let sec_hash = compute_section_hash(section);
        new_sec_fps.push(StoredSectionFingerprint {
            section_id: section.id,
            name: section.name.clone(),
            summary_hash: sec_hash.clone(),
        });

        if let Some(old_sec) = sec_map.get(&section.id) {
            if old_sec.summary_hash != sec_hash {
                changes.push(ContentChange {
                    id: 0, course_id, module_id: 0,
                    module_name: format!("Section: {}", section.name),
                    section_name: section.name.clone(),
                    change_type: "section_updated".into(),
                    old_val: String::new(), new_val: String::new(),
                    detected_at: 0,
                });
            }
        }

        for module in &section.modules {
            seen_mod_ids.insert(module.id);
            let mod_hash = compute_module_hash(module);
            new_mod_fps.push(StoredFingerprint {
                module_id: module.id,
                name: module.name.clone(),
                content_hash: mod_hash.clone(),
            });

            if let Some(old_mod) = mod_map.get(&module.id) {
                if old_mod.name != module.name {
                    changes.push(ContentChange {
                        id: 0, course_id, module_id: module.id,
                        module_name: module.name.clone(),
                        section_name: section.name.clone(),
                        change_type: "renamed".into(),
                        old_val: old_mod.name.clone(),
                        new_val: module.name.clone(),
                        detected_at: 0,
                    });
                }
                if old_mod.content_hash != mod_hash {
                    changes.push(ContentChange {
                        id: 0, course_id, module_id: module.id,
                        module_name: module.name.clone(),
                        section_name: section.name.clone(),
                        change_type: "content_updated".into(),
                        old_val: String::new(), new_val: String::new(),
                        detected_at: 0,
                    });
                }
            } else if !first_run {
                changes.push(ContentChange {
                    id: 0, course_id, module_id: module.id,
                    module_name: module.name.clone(),
                    section_name: section.name.clone(),
                    change_type: "added".into(),
                    old_val: String::new(),
                    new_val: module.name.clone(),
                    detected_at: 0,
                });
            }
        }
    }

    // Detect removals
    let removed_mod_ids: Vec<u64> = stored_modules.iter()
        .filter(|fp| !seen_mod_ids.contains(&fp.module_id))
        .map(|fp| fp.module_id).collect();
    let removed_sec_ids: Vec<u64> = stored_sections.iter()
        .filter(|fp| !seen_sec_ids.contains(&fp.section_id))
        .map(|fp| fp.section_id).collect();

    if !first_run {
        for &rid in &removed_mod_ids {
            if let Some(old) = mod_map.get(&rid) {
                changes.push(ContentChange {
                    id: 0, course_id, module_id: rid,
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

    (changes, new_mod_fps, new_sec_fps, removed_mod_ids, removed_sec_ids)
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::{CourseSection, CourseModule, ModuleContent};

    fn section(modules: Vec<CourseModule>) -> Vec<CourseSection> {
        vec![CourseSection { id: 1, name: "Week 1".into(), summary: String::new(), modules }]
    }

    fn module(id: u64, name: &str, desc: &str) -> CourseModule {
        CourseModule {
            id,
            name: name.into(),
            modname: "page".into(),
            description: if desc.is_empty() { None } else { Some(desc.into()) },
            url: None,
            contents: vec![],
            mainpage: if desc.is_empty() { None } else { Some(format!("<p>{desc}</p>")) },
            dates: vec![],
        }
    }

    fn module_with_file(id: u64, name: &str, size: u64) -> CourseModule {
        CourseModule {
            id,
            name: name.into(),
            modname: "resource".into(),
            description: None,
            url: Some("http://example.com/file.pdf".into()),
            mainpage: None,
            contents: vec![ModuleContent {
                filename: "notes.pdf".into(),
                fileurl: "http://example.com/file.pdf".into(),
                filesize: size,
                timemodified: 0,
                mimetype: Some("application/pdf".into()),
            }],
            dates: vec![],
        }
    }

    #[test]
    fn first_run_generates_no_changes() {
        let sections = section(vec![module(1, "Intro", "Hello world")]);
        let (changes, mods, secs, rem_mods, rem_secs) = diff_content(101, &sections, &[], &[]);
        assert!(changes.is_empty());
        assert_eq!(mods.len(), 1);
        assert_eq!(secs.len(), 1);
        assert!(rem_mods.is_empty());
        assert!(rem_secs.is_empty());
    }

    #[test]
    fn content_change_detected() {
        let v1 = section(vec![module(1, "Intro", "Original text here")]);
        let (_, mods, secs, _, _) = diff_content(101, &v1, &[], &[]);

        let v2 = section(vec![module(1, "Intro", "Updated text with extra content")]);
        let (changes, _, _, _, _) = diff_content(101, &v2, &mods, &secs);

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].change_type, "content_updated");
        assert_eq!(changes[0].module_id, 1);
    }

    #[test]
    fn section_summary_change_detected() {
        let mut v1 = section(vec![module(1, "Intro", "Some text")]);
        v1[0].summary = "Original summary".into();
        let (_, mods, secs, _, _) = diff_content(101, &v1, &[], &[]);

        let mut v2 = section(vec![module(1, "Intro", "Some text")]);
        v2[0].summary = "Updated summary".into();
        let (changes, _, _, _, _) = diff_content(101, &v2, &mods, &secs);

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].change_type, "section_updated");
    }

    #[test]
    fn rename_detected() {
        let v1 = section(vec![module(1, "Week 1 Notes", "Some content")]);
        let (_, mods, secs, _, _) = diff_content(101, &v1, &[], &[]);

        let v2 = section(vec![module(1, "Week 1 Notes — Updated", "Some content")]);
        let (changes, _, _, _, _) = diff_content(101, &v2, &mods, &secs);

        // Rename detection triggers two changes: 'renamed' and 'content_updated' (since name is part of hash)
        assert!(changes.iter().any(|c| c.change_type == "renamed"));
        assert!(changes.iter().any(|c| c.change_type == "content_updated"));
    }

    #[test]
    fn file_size_change_detected() {
        let v1 = section(vec![module_with_file(1, "Lecture Slides", 81920)]);
        let (_, mods, secs, _, _) = diff_content(101, &v1, &[], &[]);

        let v2 = section(vec![module_with_file(1, "Lecture Slides", 204800)]);
        let (changes, _, _, _, _) = diff_content(101, &v2, &mods, &secs);

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].change_type, "content_updated");
    }

    #[test]
    fn module_added_detected() {
        let v1 = section(vec![module(1, "Intro", "First module")]);
        let (_, mods, secs, _, _) = diff_content(101, &v1, &[], &[]);

        let v2 = section(vec![
            module(1, "Intro", "First module"),
            module(2, "Week 1 Lab", "Lab assignment"),
        ]);
        let (changes, _, _, _, _) = diff_content(101, &v2, &mods, &secs);

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].change_type, "added");
        assert_eq!(changes[0].module_id, 2);
    }

    #[test]
    fn module_removed_detected() {
        let v1 = section(vec![
            module(1, "Intro", "First module"),
            module(2, "Old Resource", "Will be removed"),
        ]);
        let (_, mods, secs, _, _) = diff_content(101, &v1, &[], &[]);

        let v2 = section(vec![module(1, "Intro", "First module")]);
        let (changes, _, _, rem_mods, _) = diff_content(101, &v2, &mods, &secs);

        assert_eq!(rem_mods.len(), 1);
        assert_eq!(rem_mods[0], 2);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].change_type, "removed");
    }

    #[test]
    fn text_diff_detects_added_line() {
        let old = "Line one\nLine two\n";
        let new = "Line one\nLine two\nLine three\n";
        let diff = text_diff(old, new);
        assert!(diff.contains("+ Line three"));
        assert!(!diff.contains("- "));
    }
}

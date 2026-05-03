use crate::api::types::Course;

pub fn parse_dept(shortname: &str) -> Option<String> {
    shortname
        .split_whitespace()
        .next()
        .filter(|s| s.chars().all(|c| c.is_alphabetic()) && !s.is_empty())
        .map(|s| s.to_uppercase())
}

pub fn parse_year(shortname: &str) -> Option<u8> {
    shortname
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.chars().next())
        .and_then(|c| c.to_digit(10))
        .map(|d| d as u8)
        .filter(|&y| y >= 1 && y <= 4)
}

pub fn infer_student_year(courses: &[Course]) -> Option<u8> {
    courses.iter().filter_map(|c| parse_year(&c.shortname)).max()
}

/// Parse student registration number from Moodle email.
/// `2023s20003@stu.cmb.ac.lk` → `S/20003/2023`
pub fn parse_student_id(email: &str) -> Option<String> {
    let local = email.split('@').next()?;
    let s_pos = local.find('s')?;
    let year = &local[..s_pos];
    let num  = &local[s_pos + 1..];
    if year.len() == 4 && year.chars().all(|c| c.is_ascii_digit()) && !num.is_empty() {
        Some(format!("S/{num}/{year}"))
    } else {
        None
    }
}

pub fn year_label(year: u8) -> &'static str {
    match year {
        1 => "1st Year",
        2 => "2nd Year",
        3 => "3rd Year",
        4 => "4th Year",
        _ => "Unknown",
    }
}

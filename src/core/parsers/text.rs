/// Cleans raw plain-text / source code by trimming redundant whitespace,
/// removing pagination marks (e.g. "Page 1 of 12"), and normalizing carriage returns.
pub fn clean_text_slice(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut consecutive_empty_lines = 0;

    for line in input.lines() {
        let trimmed = line.trim();

        // Drop common pagination artifacts & non-informative line rules
        if is_boilerplate_line(trimmed) {
            consecutive_empty_lines += 1;
            if consecutive_empty_lines <= 1 && !result.is_empty() {
                result.push('\n');
            }
            continue;
        }

        if trimmed.is_empty() {
            consecutive_empty_lines += 1;
            if consecutive_empty_lines <= 1 {
                result.push('\n');
            }
        } else {
            consecutive_empty_lines = 0;
            result.push_str(trimmed);
            result.push('\n');
        }
    }

    result
}

#[inline]
fn is_boilerplate_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.starts_with("page ")
        || lower.contains("all rights reserved")
        || lower.starts_with("copyright (c)")
        || (line.chars().all(|c| c == '-' || c == '=' || c == '_') && line.len() > 3)
}

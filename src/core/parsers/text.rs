use std::collections::HashMap;

/// Cleans raw plain-text / PDF stream text / DOCX text by:
/// 1. Stripping pagination marks (e.g. "Page 1 of 12", "- 12 -", "[ 5 ]", "1 / 40").
/// 2. Eliminating watermarks and document classification banners (e.g. "CONFIDENTIAL", "DRAFT").
/// 3. Detecting and eliminating recurring running headers and footers across pages.
/// 4. Normalizing carriage returns and redundant whitespace.
pub fn clean_text_slice(input: &str) -> String {
    // Phase 1: Identify recurring running headers and footers (lines repeating >= 2 times across pages)
    let mut candidate_line_counts: HashMap<&str, usize> = HashMap::new();
    for line in input.lines() {
        let trimmed = line.trim();
        if is_running_header_candidate(trimmed) {
            *candidate_line_counts.entry(trimmed).or_insert(0) += 1;
        }
    }

    let mut result = String::with_capacity(input.len());
    let mut consecutive_empty_lines = 0;

    // Phase 2: Filter boilerplate lines, running headers/footers, and watermarks
    for line in input.lines() {
        let trimmed = line.trim();

        // Drop recurring running headers/footers that appear on 2 or more pages
        if let Some(&count) = candidate_line_counts.get(trimmed) {
            if count >= 2 {
                consecutive_empty_lines += 1;
                if consecutive_empty_lines <= 1 && !result.is_empty() {
                    result.push('\n');
                }
                continue;
            }
        }

        // Drop pagination artifacts, watermarks, copyright notices, and line dividers
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
fn is_running_header_candidate(trimmed: &str) -> bool {
    // Running headers/footers are short, non-code, non-sentence lines
    if trimmed.len() < 3 || trimmed.len() > 100 {
        return false;
    }

    // Must not be a sentence ending with standard punctuation
    if trimmed.ends_with('.') || trimmed.ends_with('?') || trimmed.ends_with('!') || trimmed.ends_with(':') {
        return false;
    }

    // Must not be code or markdown syntax
    if trimmed.starts_with("```")
        || trimmed.starts_with("- ")
        || trimmed.starts_with("* ")
        || trimmed.starts_with("+ ")
        || trimmed.starts_with("#")
        || trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with("let ")
        || trimmed.starts_with("const ")
        || trimmed.starts_with("fn ")
    {
        return false;
    }

    true
}

#[inline]
pub fn is_boilerplate_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();

    // 1. Explicit pagination labels
    if lower.starts_with("page ") || lower.starts_with("pg. ") || lower.starts_with("pg ") {
        return true;
    }

    // 2. Bare numbers: "1", "42", "- 12 -", "[ 5 ]", "( 3 )", "— 10 —", "• 4 •"
    if is_bare_page_number(trimmed) {
        return true;
    }

    // 3. Fraction pagination: "1/10", "1 / 10", "1 of 10", "12 of 340"
    if is_fraction_pagination(&lower) {
        return true;
    }

    // 4. Running header with pipe or bullet: "Document Title | Page 1", "Page 1 | Document Title"
    if is_header_with_page(&lower) {
        return true;
    }

    // 5. Watermarks & Document Classifications
    match lower.as_str() {
        "confidential"
        | "strictly confidential"
        | "internal use only"
        | "for internal use only"
        | "proprietary and confidential"
        | "proprietary & confidential"
        | "draft"
        | "preliminary draft"
        | "do not distribute"
        | "classified"
        | "unclassified"
        | "restricted" => return true,
        _ => {}
    }

    // 6. Copyright lines & horizontal divider lines
    if lower.contains("all rights reserved")
        || lower.starts_with("copyright (c)")
        || lower.starts_with("copyright ©")
        || (trimmed.chars().all(|c| c == '-' || c == '=' || c == '_' || c == '*') && trimmed.len() > 3)
    {
        return true;
    }

    false
}

#[inline]
fn is_bare_page_number(s: &str) -> bool {
    let clean = s.trim_matches(|c: char| {
        c == '-' || c == '—' || c == '[' || c == ']' || c == '(' || c == ')' || c == '~' || c == '•' || c.is_whitespace()
    });
    !clean.is_empty() && clean.len() <= 4 && clean.chars().all(|c| c.is_ascii_digit())
}

#[inline]
fn is_fraction_pagination(s: &str) -> bool {
    // "1 of 10", "12 of 34", "1/10", "1 / 10"
    if let Some((left, right)) = s.split_once(" of ") {
        let l = left.trim();
        let r = right.trim();
        if !l.is_empty() && !r.is_empty() && l.chars().all(|c| c.is_ascii_digit()) && r.chars().all(|c| c.is_ascii_digit()) {
            return true;
        }
    }
    if let Some((left, right)) = s.split_once('/') {
        let l = left.trim();
        let r = right.trim();
        if !l.is_empty() && !r.is_empty() && l.chars().all(|c| c.is_ascii_digit()) && r.chars().all(|c| c.is_ascii_digit()) {
            return true;
        }
    }
    false
}

#[inline]
fn is_header_with_page(s: &str) -> bool {
    if s.contains('|') || s.contains('—') {
        let parts: Vec<&str> = s.split(|c| c == '|' || c == '—').map(|p| p.trim()).collect();
        for p in parts {
            if p.starts_with("page ") || is_bare_page_number(p) || is_fraction_pagination(p) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bare_page_numbers_and_fractions() {
        assert!(is_boilerplate_line("42"));
        assert!(is_boilerplate_line("- 12 -"));
        assert!(is_boilerplate_line("[ 5 ]"));
        assert!(is_boilerplate_line("— 9 —"));
        assert!(is_boilerplate_line("1 of 24"));
        assert!(is_boilerplate_line("3 / 10"));
        assert!(is_boilerplate_line("Page 4 of 12"));
        assert!(is_boilerplate_line("Annual Report 2024 | Page 12"));

        assert!(!is_boilerplate_line("In 2024, the team completed 5 projects."));
        assert!(!is_boilerplate_line("Chapter 1: The Beginning"));
    }

    #[test]
    fn test_watermarks_and_classifications() {
        assert!(is_boilerplate_line("CONFIDENTIAL"));
        assert!(is_boilerplate_line("STRICTLY CONFIDENTIAL"));
        assert!(is_boilerplate_line("FOR INTERNAL USE ONLY"));
        assert!(is_boilerplate_line("DRAFT"));
        assert!(is_boilerplate_line("DO NOT DISTRIBUTE"));
    }

    #[test]
    fn test_running_headers_removal() {
        let doc = r#"
ACME System Architecture Specification
Chapter 1: Overview
This is the overview of the system architecture.
1

ACME System Architecture Specification
Chapter 2: Data Pipeline
This is the explanation of the data pipeline.
2
        "#;

        let cleaned = clean_text_slice(doc);
        assert!(!cleaned.contains("ACME System Architecture Specification"));
        assert!(!cleaned.contains("\n1\n"));
        assert!(!cleaned.contains("\n2\n"));
        assert!(cleaned.contains("Chapter 1: Overview"));
        assert!(cleaned.contains("Chapter 2: Data Pipeline"));
    }
}

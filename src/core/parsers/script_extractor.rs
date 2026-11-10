use serde_json::Value;
use std::collections::HashSet;

/// Extracts structured text content stored inside `<script>` tags:
/// 1. JSON-LD / JSON hydration blobs (`type="application/ld+json"`, Next.js `__NEXT_DATA__`, Nuxt, etc.)
/// 2. Inline JavaScript data arrays and object records (e.g., `const projects = [{ name: "...", description: "..." }]`)
/// 3. Embedded Markdown strings or template literals (e.g., `const content = \`# Heading\n...\``)
pub fn extract_inline_script_content(html_raw: &str) -> String {
    let mut extracted_sections: Vec<String> = Vec::new();
    let mut seen_snippets: HashSet<String> = HashSet::new();

    // Iterate through all <script>...</script> tags
    for (attrs, script_body) in find_script_tags(html_raw) {
        let script_trim = script_body.trim();
        if script_trim.is_empty() {
            continue;
        }

        // Case 1: JSON / JSON-LD / Hydration data
        if attrs.contains("application/ld+json")
            || attrs.contains("application/json")
            || attrs.contains("__NEXT_DATA__")
            || attrs.contains("__NUXT_DATA__")
        {
            if let Ok(json) = serde_json::from_str::<Value>(script_trim) {
                let mut json_text = String::new();
                extract_text_from_json(&json, &mut json_text, &mut seen_snippets);
                let trimmed = json_text.trim();
                if !trimmed.is_empty() {
                    extracted_sections.push(trimmed.to_string());
                }
            }
            continue;
        }

        // Case 2: Inline JS object arrays (e.g. const projects = [{ name: "...", description: "..." }])
        let records = extract_js_object_records(script_trim);
        for record in records {
            let record_md = format_record_as_markdown(&record);
            if !record_md.is_empty() {
                let key = record.get("name").or_else(|| record.get("title")).cloned().unwrap_or_default();
                if !key.is_empty() && seen_snippets.insert(key) {
                    extracted_sections.push(record_md);
                }
            }
        }

        // Case 3: Embedded Markdown documents or long prose template literals (`...`)
        for template in extract_markdown_templates(script_trim) {
            let sample = template.chars().take(80).collect::<String>();
            if seen_snippets.insert(sample) {
                extracted_sections.push(template);
            }
        }
    }

    if extracted_sections.is_empty() {
        String::new()
    } else {
        extracted_sections.join("\n\n")
    }
}

/// Finds all `<script>` tags and returns pairs of (attributes, inner_content)
fn find_script_tags(html: &str) -> Vec<(String, String)> {
    let mut results = Vec::new();
    let mut cursor = 0;
    let html_lower = html.to_ascii_lowercase();

    while let Some(open_tag_start) = html_lower[cursor..].find("<script") {
        let abs_open_start = cursor + open_tag_start;
        let open_tag_end = match html[abs_open_start..].find('>') {
            Some(idx) => abs_open_start + idx,
            None => break,
        };

        let attrs = html[abs_open_start + 7..open_tag_end].to_string();

        let close_tag_start = match html_lower[open_tag_end..].find("</script") {
            Some(idx) => open_tag_end + idx,
            None => html.len(),
        };

        let body = html[open_tag_end + 1..close_tag_start].to_string();
        results.push((attrs, body));

        cursor = match html[close_tag_start..].find('>') {
            Some(idx) => close_tag_start + idx + 1,
            None => html.len(),
        };

        if cursor >= html.len() {
            break;
        }
    }

    results
}

/// Recursively extracts readable text fields from JSON-LD and page hydration objects
fn extract_text_from_json(val: &Value, out: &mut String, seen: &mut HashSet<String>) {
    match val {
        Value::Object(map) => {
            // Check for schema.org article / blog / product fields
            let title = map
                .get("headline")
                .or_else(|| map.get("name"))
                .or_else(|| map.get("title"))
                .and_then(|v| v.as_str());

            let body = map
                .get("articleBody")
                .or_else(|| map.get("description"))
                .or_else(|| map.get("text"))
                .or_else(|| map.get("summary"))
                .and_then(|v| v.as_str());

            if let Some(t) = title {
                let clean_t = t.trim();
                if clean_t.len() > 3 && seen.insert(clean_t.to_string()) {
                    out.push_str("\n\n### ");
                    out.push_str(clean_t);
                    out.push('\n');
                }
            }

            if let Some(b) = body {
                let clean_b = b.trim();
                if clean_b.len() > 15 && seen.insert(clean_b.to_string()) {
                    out.push_str(clean_b);
                    out.push_str("\n\n");
                }
            }

            // Recurse into child values
            for (_, v) in map {
                extract_text_from_json(v, out, seen);
            }
        }
        Value::Array(arr) => {
            for item in arr {
                extract_text_from_json(item, out, seen);
            }
        }
        _ => {}
    }
}

/// Extracts object records from inline JS code (e.g. `{ name: "...", description: "..." }`)
fn extract_js_object_records(script: &str) -> Vec<std::collections::HashMap<String, String>> {
    let mut records = Vec::new();
    let bytes = script.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'{' {
            let start = i;
            let mut depth = 0;
            let mut in_str = false;
            let mut quote = 0;
            let mut end = None;

            while i < len {
                let b = bytes[i];
                if in_str {
                    if b == b'\\' && i + 1 < len {
                        i += 2;
                        continue;
                    }
                    if b == quote {
                        in_str = false;
                    }
                } else if b == b'"' || b == b'\'' || b == b'`' {
                    in_str = true;
                    quote = b;
                } else if b == b'{' {
                    depth += 1;
                } else if b == b'}' {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(i + 1);
                        break;
                    }
                }
                i += 1;
            }

            if let Some(end_idx) = end {
                if let Ok(obj_str) = std::str::from_utf8(&bytes[start..end_idx]) {
                    let kvs = parse_flat_js_object(obj_str);
                    // Only keep records that look like substantive human content
                    if has_human_content(&kvs) {
                        records.push(kvs);
                    }
                }
            }
        } else {
            i += 1;
        }
    }

    records
}

/// Parses simple key-value pairs from a JS object literal string
fn parse_flat_js_object(obj_str: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let chars: Vec<char> = obj_str.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Find identifier
        if chars[i].is_alphabetic() || chars[i] == '_' || chars[i] == '$' {
            let key_start = i;
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '$') {
                i += 1;
            }
            let key: String = chars[key_start..i].iter().collect();

            // Skip whitespace to colon
            while i < len && chars[i].is_whitespace() {
                i += 1;
            }

            if i < len && chars[i] == ':' {
                i += 1;
                while i < len && chars[i].is_whitespace() {
                    i += 1;
                }

                // Check if string literal follows
                if i < len && (chars[i] == '"' || chars[i] == '\'' || chars[i] == '`') {
                    let quote = chars[i];
                    i += 1;
                    let mut val = String::new();
                    while i < len {
                        if chars[i] == '\\' && i + 1 < len {
                            val.push(chars[i + 1]);
                            i += 2;
                            continue;
                        }
                        if chars[i] == quote {
                            i += 1;
                            break;
                        }
                        val.push(chars[i]);
                        i += 1;
                    }
                    map.insert(key, val);
                    continue;
                }
            }
        }
        i += 1;
    }

    map
}

/// Checks if a parsed JS object contains substantive content fields
fn has_human_content(kvs: &std::collections::HashMap<String, String>) -> bool {
    let has_title_or_name = kvs.contains_key("name")
        || kvs.contains_key("title")
        || kvs.contains_key("headline")
        || kvs.contains_key("label");

    let has_body_or_desc = kvs.contains_key("description")
        || kvs.contains_key("summary")
        || kvs.contains_key("role")
        || kvs.contains_key("bio")
        || kvs.contains_key("content")
        || kvs.contains_key("text");

    if has_title_or_name && has_body_or_desc {
        return true;
    }

    // Alternatively, any single field with substantial prose (> 40 chars)
    kvs.values().any(|v| v.len() > 40 && v.contains(' '))
}

/// Formats a discovered JS content record as clean Markdown
fn format_record_as_markdown(record: &std::collections::HashMap<String, String>) -> String {
    let title = record
        .get("name")
        .or_else(|| record.get("title"))
        .or_else(|| record.get("headline"))
        .cloned();

    let description = record
        .get("description")
        .or_else(|| record.get("summary"))
        .or_else(|| record.get("content"))
        .or_else(|| record.get("text"))
        .cloned();

    let role = record.get("role").or_else(|| record.get("category")).cloned();
    let link = record
        .get("linkurl")
        .or_else(|| record.get("url"))
        .or_else(|| record.get("href"))
        .cloned();
    let link_name = record.get("linkname").cloned().unwrap_or_else(|| "Link".to_string());

    let mut md = String::new();

    if let Some(t) = title {
        md.push_str("### ");
        md.push_str(&t);
        md.push('\n');
    }

    if let Some(r) = role {
        md.push_str("**Role:** ");
        md.push_str(&r);
        md.push_str("\n\n");
    }

    if let Some(d) = description {
        md.push_str(&d);
        md.push('\n');
    }

    if let Some(l) = link {
        if l.starts_with("http://") || l.starts_with("https://") || l.starts_with('/') {
            md.push_str(&format!("\n[{}]({})\n", link_name, l));
        }
    }

    md.trim().to_string()
}

/// Finds embedded Markdown documents inside template literals (``` `...` ```)
fn extract_markdown_templates(script: &str) -> Vec<String> {
    let mut templates = Vec::new();
    let bytes = script.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'`' {
            let start = i + 1;
            i += 1;
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'`' {
                    let end = i;
                    if let Ok(content) = std::str::from_utf8(&bytes[start..end]) {
                        let trimmed = content.trim();
                        // Check if template contains significant Markdown structure
                        if is_markdown_document(trimmed) {
                            templates.push(trimmed.to_string());
                        }
                    }
                    break;
                }
                i += 1;
            }
        }
        i += 1;
    }

    templates
}

/// Checks if a string template literal is an authentic Markdown document or prose
fn is_markdown_document(trimmed: &str) -> bool {
    if trimmed.len() < 80 {
        return false;
    }
    // Discard code definitions and regex/keyword bags
    if trimmed.starts_with("function")
        || trimmed.starts_with("export")
        || trimmed.starts_with("import")
        || trimmed.contains("case_insensitive")
        || trimmed.contains("keywords:")
        || trimmed.contains("className:")
        || trimmed.contains("C_NUMBER_MODE")
    {
        return false;
    }

    let mut heading_count = 0;
    let mut list_count = 0;
    let has_blank_lines = trimmed.contains("\n\n");

    for line in trimmed.lines() {
        let l = line.trim();
        if l.starts_with("# ") || l.starts_with("## ") || l.starts_with("### ") {
            heading_count += 1;
        } else if l.starts_with("- ") || l.starts_with("* ") || l.starts_with("+ ") {
            list_count += 1;
        }
    }

    (heading_count >= 1 && (list_count >= 1 || has_blank_lines)) || (heading_count >= 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_js_project_array() {
        let html = r#"
            <script>
                const projects = [
                    {
                        name: "The Remains of My Affection",
                        role: "Creator, writer",
                        description: "The nightmares that have been haunting Lisa for weeks.",
                        linkurl: "https://adam-w.itch.io/the-remains-of-my-affection",
                        linkname: "Itch.io"
                    }
                ];
            </script>
        "#;
        let md = extract_inline_script_content(html);
        assert!(md.contains("### The Remains of My Affection"));
        assert!(md.contains("**Role:** Creator, writer"));
        assert!(md.contains("The nightmares that have been haunting Lisa"));
        assert!(md.contains("[Itch.io](https://adam-w.itch.io/the-remains-of-my-affection)"));
    }

    #[test]
    fn test_extract_json_ld() {
        let html = r#"
            <script type="application/ld+json">
            {
                "@context": "https://schema.org",
                "@type": "NewsArticle",
                "headline": "Rust 2026 Breakthrough",
                "articleBody": "High performance token optimization engine announced."
            }
            </script>
        "#;
        let md = extract_inline_script_content(html);
        assert!(md.contains("### Rust 2026 Breakthrough"));
        assert!(md.contains("High performance token optimization engine announced."));
    }

    #[test]
    fn test_extract_markdown_template_literal() {
        let html = r#"
            <script>
                const is = `
# Markdown Header
- Item 1
- Item 2

This is a comprehensive demo document embedded directly into the JS bundle.
`;
            </script>
        "#;
        let md = extract_inline_script_content(html);
        assert!(md.contains("# Markdown Header"));
        assert!(md.contains("- Item 1"));
    }
}

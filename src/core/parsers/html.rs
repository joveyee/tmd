use crate::error::AppError;
use tl::{Node, ParserOptions};

/// Zero-copy DOM parser stripping scripts, styles, navigations, and extracting main text
pub fn extract_article_markdown(html_raw: &str) -> Result<String, AppError> {
    let dom = tl::parse(html_raw, ParserOptions::default())
        .map_err(|e| AppError::HtmlParse(e.to_string()))?;
    let parser = dom.parser();

    let mut buffer = String::with_capacity(html_raw.len() / 2);

    // tl query_selector takes single selectors, not comma-separated lists
    let content_selectors = ["article", "main", "[role='main']", "body"];
    let mut root_handle = None;

    for selector in content_selectors {
        if let Some(mut iter) = dom.query_selector(selector) {
            if let Some(handle) = iter.next() {
                root_handle = Some(handle);
                break;
            }
        }
    }

    if let Some(handle) = root_handle {
        if let Some(node) = handle.get(parser) {
            traverse_and_build(node, parser, &mut buffer, false);
        }
    } else {
        // Fallback: traverse all top-level DOM nodes if no container tag was found
        for node in dom.children() {
            if let Some(n) = node.get(parser) {
                traverse_and_build(n, parser, &mut buffer, false);
            }
        }
    }

    let mut cleaned = buffer.trim().to_string();

    // Extract rich content embedded inside inline scripts (JSON-LD, Next.js hydration, JS data arrays, template literals)
    let script_content = crate::core::parsers::script_extractor::extract_inline_script_content(html_raw);
    if !script_content.is_empty() {
        if cleaned.is_empty() {
            cleaned = script_content;
        } else {
            cleaned.push_str("\n\n---\n\n");
            cleaned.push_str(&script_content);
        }
    }

    Ok(cleaned)
}

fn traverse_and_build(node: &Node, parser: &tl::Parser, out: &mut String, inside_pre: bool) {
    match node {
        Node::Tag(tag) => {
            let name = tag.name().as_utf8_str();

            // Skip uninformative or noisy tags
            match name.as_ref() {
                "script" | "style" | "noscript" | "svg" | "nav" | "footer" 
                | "header" | "aside" | "iframe" | "button" | "input" | "select" => return,
                "h1" => out.push_str("\n\n# "),
                "h2" => out.push_str("\n\n## "),
                "h3" => out.push_str("\n\n### "),
                "h4" => out.push_str("\n\n#### "),
                "h5" => out.push_str("\n\n##### "),
                "h6" => out.push_str("\n\n###### "),
                "p" => out.push_str("\n\n"),
                "li" => out.push_str("\n- "),
                "pre" => out.push_str("\n```\n"),
                "code" if !inside_pre => {
                    if !out.is_empty() && !out.ends_with(' ') && !out.ends_with('\n') {
                        out.push(' ');
                    }
                    out.push('`');
                }
                "a" => {
                    if !out.is_empty() && !out.ends_with(' ') && !out.ends_with('\n') && !out.ends_with('(') {
                        out.push(' ');
                    }
                    out.push('[');
                }
                _ => {}
            }

            let is_pre = inside_pre || name.as_ref() == "pre";

            for child in tag.children().top().iter() {
                if let Some(child_node) = child.get(parser) {
                    traverse_and_build(child_node, parser, out, is_pre);
                }
            }

            match name.as_ref() {
                "pre" => {
                    if out.ends_with(' ') {
                        out.pop();
                    }
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push_str("```\n");
                }
                "code" if !inside_pre => {
                    if out.ends_with(' ') {
                        out.pop();
                    }
                    out.push('`');
                }
                "a" => {
                    if out.ends_with(' ') {
                        out.pop();
                    }
                    if let Some(Some(href)) = tag.attributes().get("href") {
                        let href_str = href.as_utf8_str();
                        let href_trim = href_str.trim();
                        if !href_trim.is_empty() && !href_trim.starts_with('#') && !href_trim.starts_with("javascript:") {
                            out.push_str(&format!("]({})", href_trim));
                        } else {
                            out.push(']');
                        }
                    } else {
                        out.push(']');
                    }
                }
                _ => {}
            }
        }
        Node::Raw(bytes) => {
            if let Ok(text) = std::str::from_utf8(bytes.as_bytes()) {
                let clean = text.trim();
                if !clean.is_empty() {
                    let decoded = decode_html_entities(clean);
                    out.push_str(&decoded);
                    out.push(' ');
                }
            }
        }
        _ => {}
    }
}

fn decode_html_entities(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_parsing_headings_and_lists() {
        let html = r#"
            <html>
                <body>
                    <h1>Main Title</h1>
                    <p>Introduction paragraph with <code>inline code</code>.</p>
                    <h2>Subheading</h2>
                    <ul>
                        <li>Item 1</li>
                        <li>Item 2</li>
                    </ul>
                    <pre><code>let x = 42;</code></pre>
                </body>
            </html>
        "#;
        let md = extract_article_markdown(html).unwrap();
        assert!(md.contains("# Main Title"));
        assert!(md.contains("`inline code`"));
        assert!(md.contains("## Subheading"));
        assert!(md.contains("- Item 1"));
        assert!(md.contains("```\nlet x = 42;\n```"));
        // Ensure no doubled ```
        assert!(!md.contains("```\n```"));
    }

    #[test]
    fn test_html_entities_decoding() {
        let html = "<p>Tom &amp; Jerry &gt; Sylvester &amp; Tweety</p>";
        let md = extract_article_markdown(html).unwrap();
        assert!(md.contains("Tom & Jerry > Sylvester & Tweety"));
    }

    #[test]
    fn test_fragment_without_body() {
        let html = "<div><h3>Fragment Header</h3><p>Fragment text</p></div>";
        let md = extract_article_markdown(html).unwrap();
        assert!(md.contains("### Fragment Header"));
        assert!(md.contains("Fragment text"));
    }
}

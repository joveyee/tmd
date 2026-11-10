use crate::error::AppError;
use tl::{Node, ParserOptions};

/// Zero-copy DOM parser stripping scripts, styles, navigations, and extracting main text
pub fn extract_article_markdown(html_raw: &str) -> Result<String, AppError> {
    let dom = tl::parse(html_raw, ParserOptions::default())
        .map_err(|e| AppError::HtmlParse(e.to_string()))?;
    let parser = dom.parser();

    let mut buffer = String::with_capacity(html_raw.len() / 2);
    
    // Attempt to locate article/main tag; fallback to body
    let root_handles = dom
        .query_selector("article, main, [role='main']")
        .and_then(|mut iter| iter.next())
        .or_else(|| dom.query_selector("body").and_then(|mut iter| iter.next()));

    if let Some(handle) = root_handles {
        if let Some(node) = handle.get(parser) {
            traverse_and_build(node, parser, &mut buffer);
        }
    } else {
        // Fallback: full tree traversal
        for node in dom.children() {
            if let Some(n) = node.get(parser) {
                traverse_and_build(n, parser, &mut buffer);
            }
        }
    }

    Ok(buffer)
}

fn traverse_and_build(node: &Node, parser: &tl::Parser, out: &mut String) {
    match node {
        Node::Tag(tag) => {
            let name = tag.name().as_utf8_str();
            // Blacklist uninformative or noisy tags
            match name.as_ref() {
                "script" | "style" | "noscript" | "svg" | "nav" | "footer" | "header"
                | "aside" | "iframe" => return,
                "h1" => out.push_str("\n\n# "),
                "h2" => out.push_str("\n\n## "),
                "h3" => out.push_str("\n\n### "),
                "p" => out.push_str("\n\n"),
                "li" => out.push_str("\n- "),
                "pre" | "code" => out.push_str("\n```\n"),
                _ => {}
            }

            for child in tag.children().top().iter() {
                if let Some(child_node) = child.get(parser) {
                    traverse_and_build(child_node, parser, out);
                }
            }

            if matches!(name.as_ref(), "pre" | "code") {
                out.push_str("\n```\n");
            }
        }
        Node::Raw(bytes) => {
            if let Ok(text) = std::str::from_utf8(bytes.as_bytes()) {
                let clean = text.trim();
                if !clean.is_empty() {
                    out.push_str(clean);
                    out.push(' ');
                }
            }
        }
        _ => {}
    }
}

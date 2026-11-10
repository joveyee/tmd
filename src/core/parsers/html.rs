use crate::error::AppError;
use tl::{Node, ParserOptions};

/// Zero-copy DOM parser stripping scripts, styles, navigations, and extracting main text
pub fn extract_article_markdown(html_raw: &str) -> Result<String, AppError> {
    let dom = tl::parse(html_raw, ParserOptions::default())
        .map_err(|e| AppError::HtmlParse(e.to_string()))?;
    let parser = dom.parser();

    let mut buffer = String::with_capacity(html_raw.len() / 2);

    // Prioritize main content containers so forum discussions & comments are captured
    let content_selectors = [
        "main",
        "[role='main']",
        "article",
        "[itemprop='articleBody']",
        ".article-body",
        ".article-content",
        ".post-content",
        ".entry-content",
        ".markdown-body",
        "body",
    ];
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

    // Capture discussion comments if they sit outside the main post container (e.g. Reddit, modern forums)
    let comment_selectors = ["shreddit-comment", "#comment-tree", ".comment-tree", "#comments", ".comments-area"];
    for sel in comment_selectors {
        if let Some(iter) = dom.query_selector(sel) {
            let mut found_any = false;
            for handle in iter {
                if let Some(node) = handle.get(parser) {
                    let mut comment_chunk = String::new();
                    traverse_and_build(node, parser, &mut comment_chunk, false);
                    let trimmed = comment_chunk.trim();
                    if !trimmed.is_empty() && !buffer.contains(trimmed) {
                        if !found_any {
                            buffer.push_str("\n\n## Comments\n\n");
                            found_any = true;
                        }
                        buffer.push_str(trimmed);
                        buffer.push_str("\n\n");
                    }
                }
                if sel == "#comment-tree" || sel == ".comment-tree" || sel == "#comments" || sel == ".comments-area" {
                    break;
                }
            }
            if found_any {
                break;
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
            // Drop ads, banners, popups, cookie consent notices, sidebars, and non-content elements
            if is_noise_or_ad_tag(tag) {
                return;
            }

            let name = tag.name().as_utf8_str();

            match name.as_ref() {
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
        .replace("&#x27;", "'")
        .replace("&apos;", "'")
        .replace("&#x22;", "\"")
        .replace("&nbsp;", " ")
}

fn is_noise_or_ad_tag(tag: &tl::HTMLTag) -> bool {
    let name = tag.name().as_utf8_str();
    let name_ref = name.as_ref();

    // 1. Non-content standard HTML tags
    match name_ref {
        "script" | "style" | "noscript" | "svg" | "canvas" | "iframe" | "frame"
        | "nav" | "footer" | "header" | "aside" | "dialog" | "button" | "input"
        | "select" | "textarea" | "form" => return true,
        _ => {}
    }

    // 2. Custom ad web components (Reddit, AMP, etc.)
    if name_ref.contains("-ad-")
        || name_ref.ends_with("-ad")
        || name_ref.starts_with("ad-")
        || name_ref.contains("banner")
        || name_ref.contains("sponsor")
    {
        return true;
    }

    // 3. ARIA roles for non-content UI landmarks (headers, navigation, sidebars, footers)
    if let Some(Some(role)) = tag.attributes().get("role") {
        let r = role.as_utf8_str().to_ascii_lowercase();
        if matches!(r.as_str(), "banner" | "navigation" | "complementary" | "contentinfo") {
            return true;
        }
    }

    // 4. Class inspection for ads, cookie banners, popups, and social bars
    if let Some(Some(class_bytes)) = tag.attributes().get("class") {
        let c = class_bytes.as_utf8_str().to_ascii_lowercase();
        if contains_non_content_marker(&c) {
            return true;
        }
    }

    // 5. ID inspection
    if let Some(Some(id_bytes)) = tag.attributes().get("id") {
        let id_str = id_bytes.as_utf8_str().to_ascii_lowercase();
        if contains_non_content_marker(&id_str) {
            return true;
        }
    }

    // 6. Test ID / Ad attributes
    if let Some(Some(testid)) = tag.attributes().get("data-testid") {
        let t = testid.as_utf8_str().to_ascii_lowercase();
        if t.contains("ad-") || t.contains("sponsor") || t.contains("promo") {
            return true;
        }
    }

    false
}

fn contains_non_content_marker(s: &str) -> bool {
    // Exact or separated tokens
    for token in s.split(|c: char| c.is_whitespace() || c == '_' || c == '-') {
        match token {
            "ad" | "ads" | "advert" | "advertisement" | "advertising"
            | "sponsored" | "sponsor" | "promoted" | "promotedlink"
            | "taboola" | "outbrain" | "adsbygoogle" | "criteo" | "carbonads"
            | "cookie" | "cookies" | "gdpr" | "consent"
            | "popup" | "modal" | "overlay" | "toast"
            | "share" | "sharing" | "sharebar" | "socialshare"
            | "newsletter" | "subscribe" | "signup"
            | "sidebar" | "breadcrumb" | "breadcrumbs" | "pagination" | "pager" => return true,
            _ => {}
        }
    }

    // Compound phrases
    s.contains("ad-container")
        || s.contains("ad-box")
        || s.contains("ad-unit")
        || s.contains("ad-banner")
        || s.contains("ad-wrapper")
        || s.contains("ad-slot")
        || s.contains("banner-ad")
        || s.contains("cookie-banner")
        || s.contains("cookie-notice")
        || s.contains("social-share")
        || s.contains("share-buttons")
        || s.contains("share-bar")
        || s.contains("newsletter-signup")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reddit_post_strips_ads_and_keeps_comments() {
        let html = r#"
            <main>
                <h1>What are your goals for Rust in 2024?</h1>
                <p>I want to finish rewriting my web server in async Rust.</p>

                <!-- Reddit Advertisement Block -->
                <shreddit-ad-post>
                    <h2>Special Offer from Cloud Sponsor!</h2>
                    <p>Buy cloud servers now at 50% discount.</p>
                </shreddit-ad-post>

                <div class="promotedlink">
                    <p>Sponsored gaming promotion ad.</p>
                </div>

                <!-- Actual Thread Comments -->
                <div class="comment">
                    <p>My goal is to learn macro_rules! and proc-macros.</p>
                </div>
            </main>
        "#;

        let md = extract_article_markdown(html).unwrap();
        assert!(md.contains("# What are your goals for Rust in 2024?"));
        assert!(md.contains("I want to finish rewriting my web server"));
        assert!(md.contains("My goal is to learn macro_rules!"));
        
        // Ensure advertisements are completely stripped
        assert!(!md.contains("Special Offer from Cloud Sponsor"));
        assert!(!md.contains("50% discount"));
        assert!(!md.contains("Sponsored gaming promotion ad"));
    }

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

    #[test]
    fn test_reddittest_html_file() {
        if let Ok(html) = std::fs::read_to_string("target/release/reddittest.html") {
            let md = extract_article_markdown(&html).unwrap();
            assert!(md.contains("# Post some really simple pure HTML Websites"));
            assert!(md.contains("## Comments"));
            assert!(md.contains("1mb.club"));
        }
    }

    #[test]
    fn test_tl_custom_element() {
        let html = "<shreddit-app><p>Hello world</p></shreddit-app>";
        let dom = tl::parse(html, ParserOptions::default()).unwrap();
        let handle = dom.query_selector("shreddit-app").and_then(|mut i| i.next());
        assert!(handle.is_some());
    }

    #[test]
    fn test_strip_cookie_sidebar_and_newsletter_widgets() {
        let html = r#"
            <body>
                <header class="site-header">
                    <nav class="nav-menu">Home | About | Blog</nav>
                </header>
                <div class="cookie-banner">
                    <p>We use cookies to improve your browsing experience. Accept all cookies.</p>
                </div>
                <div role="complementary" class="sidebar">
                    <h3>Popular Posts</h3>
                    <p>Sidebar link 1</p>
                </div>
                <article class="entry-content">
                    <h1>Real Article Content</h1>
                    <p>This is the actual article paragraph that should be preserved.</p>
                </article>
                <div class="newsletter-signup">
                    <p>Subscribe to our newsletter for weekly updates!</p>
                </div>
                <footer role="contentinfo">
                    <p>Copyright 2026. All rights reserved.</p>
                </footer>
            </body>
        "#;
        let md = extract_article_markdown(html).unwrap();
        assert!(md.contains("# Real Article Content"));
        assert!(md.contains("This is the actual article paragraph that should be preserved."));

        // Verify non-content noise and widgets are stripped
        assert!(!md.contains("Accept all cookies"));
        assert!(!md.contains("Popular Posts"));
        assert!(!md.contains("Subscribe to our newsletter"));
        assert!(!md.contains("Home | About | Blog"));
    }
}

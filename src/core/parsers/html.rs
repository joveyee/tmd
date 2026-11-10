use crate::error::AppError;
use std::collections::{HashMap, HashSet};
use tl::{Node, NodeHandle, ParserOptions};

/// Content density metrics for heuristic readability scoring
#[derive(Default, Clone, Debug)]
pub struct NodeMetrics {
    pub text_len: usize,
    pub link_text_len: usize,
    pub tag_count: usize,
    pub p_count: usize,
    pub punct_count: usize,
}

impl NodeMetrics {
    pub fn score(&self, tag_boost: f64) -> f64 {
        if self.text_len < 20 {
            return 0.0;
        }

        let link_density = self.link_text_len as f64 / self.text_len as f64;
        // Navigation bars, footer link lists, tag clouds have very high link density (>= 50%)
        if link_density >= 0.50 {
            return 0.0;
        }

        let content_text = (self.text_len.saturating_sub(self.link_text_len)) as f64;
        if content_text < 10.0 {
            return 0.0;
        }

        // Sentence punctuation bonus (periods, commas, colons, question/exclamation marks)
        let punct_factor = 1.0 + (self.punct_count as f64).min(50.0) / 25.0;

        // Paragraph and block element bonus (p, li, headings, pre, blockquote)
        let p_factor = 1.0 + (self.p_count as f64).sqrt();

        // Tag density factor: penalize deep boilerplate DOMs with very few chars per tag
        let chars_per_tag = self.text_len as f64 / (self.tag_count.max(1) as f64);
        let density_factor = (chars_per_tag / 15.0).clamp(0.5, 2.0);

        content_text * (1.0 - link_density) * p_factor * punct_factor * density_factor * tag_boost
    }
}

/// Zero-copy DOM parser stripping scripts, styles, navigations, and extracting main text
/// using heuristic content density scoring (Mozilla Readability / Boilerpipe model)
pub fn extract_article_markdown(html_raw: &str) -> Result<String, AppError> {
    let dom = tl::parse(html_raw, ParserOptions::default())
        .map_err(|e| AppError::HtmlParse(e.to_string()))?;
    let parser = dom.parser();

    let mut metrics_map: HashMap<NodeHandle, NodeMetrics> = HashMap::new();
    let mut dom_order: Vec<NodeHandle> = Vec::new();

    // 1. Compute density metrics across the content root (body or dom.children)
    // 1. Compute density metrics across content roots: body, streamed templates, or top-level children
    let mut roots: Vec<NodeHandle> = Vec::new();
    if let Some(bh) = dom.query_selector("body").and_then(|mut iter| iter.next()) {
        roots.push(bh);
    } else {
        roots.extend(dom.children());
    }

    // Streamed SSR templates (Next.js, Lit, Faceplate, React Server streaming)
    if let Some(template_iter) = dom.query_selector("template") {
        for th in template_iter {
            roots.push(th);
        }
    }

    for root_handle in roots {
        compute_node_metrics(root_handle, parser, false, &mut metrics_map, &mut dom_order);
    }

    // 2. Score candidate container blocks
    let mut candidates: Vec<(NodeHandle, f64, usize)> = Vec::new();
    for &handle in &dom_order {
        if let Some(node) = handle.get(parser) {
            if let Node::Tag(tag) = node {
                if let Some(metrics) = metrics_map.get(&handle) {
                    let boost = calculate_tag_boost(tag);
                    let score = metrics.score(boost);
                    if score > 0.0 {
                        candidates.push((handle, score, metrics.text_len));
                    }
                }
            }
        }
    }

    // Sort candidates by score descending
    candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let max_score = candidates.first().map(|c| c.1).unwrap_or(0.0);
    let mut selected_handles: Vec<NodeHandle> = Vec::new();

    if max_score >= 30.0 {
        // Collect precomputed descendant sets to efficiently check ancestor/descendant relationships
        let mut descendant_cache: HashMap<NodeHandle, HashSet<NodeHandle>> = HashMap::new();
        for (candidate_handle, _, _) in &candidates {
            descendant_cache.insert(*candidate_handle, collect_descendants(*candidate_handle, parser));
        }

        let threshold = 40.0;

        for (candidate_handle, score, _) in &candidates {
            if *score < threshold {
                continue;
            }

            let cand_descendants = match descendant_cache.get(candidate_handle) {
                Some(d) => d,
                None => continue,
            };

            let mut overlaps = false;
            for selected in &selected_handles {
                let sel_descendants = match descendant_cache.get(selected) {
                    Some(d) => d,
                    None => continue,
                };

                // If candidate is inside already selected node, or candidate contains already selected node
                if sel_descendants.contains(candidate_handle) || cand_descendants.contains(selected) {
                    overlaps = true;
                    break;
                }
            }

            if !overlaps {
                selected_handles.push(*candidate_handle);
            }
        }

        // Preserve DOM document order when rendering the selected content clusters
        selected_handles.sort_by_key(|h| {
            dom_order.iter().position(|x| x == h).unwrap_or(usize::MAX)
        });
    }

    let mut buffer = String::with_capacity(html_raw.len() / 2);

    if !selected_handles.is_empty() {
        for handle in selected_handles {
            if let Some(node) = handle.get(parser) {
                let mut chunk = String::new();
                traverse_and_build(node, parser, &mut chunk, false);
                let trimmed = chunk.trim();
                if !trimmed.is_empty() {
                    if !buffer.is_empty() {
                        buffer.push_str("\n\n");
                    }
                    buffer.push_str(trimmed);
                }
            }
        }
    } else {
        // Fallback: If no high-density cluster detected, render body or top-level nodes
        let root_handle = dom.query_selector("body").and_then(|mut iter| iter.next());
        if let Some(handle) = root_handle {
            if let Some(node) = handle.get(parser) {
                traverse_and_build(node, parser, &mut buffer, false);
            }
        } else {
            for node in dom.children() {
                if let Some(n) = node.get(parser) {
                    traverse_and_build(n, parser, &mut buffer, false);
                }
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

fn compute_node_metrics(
    handle: NodeHandle,
    parser: &tl::Parser,
    inside_link: bool,
    metrics_map: &mut HashMap<NodeHandle, NodeMetrics>,
    dom_order: &mut Vec<NodeHandle>,
) -> NodeMetrics {
    let mut current = NodeMetrics::default();

    if let Some(node) = handle.get(parser) {
        match node {
            Node::Tag(tag) => {
                if is_noise_or_ad_tag(tag) {
                    // Drop entire subtree of noise, ads, dialogs, navbars
                    return current;
                }

                current.tag_count = 1;
                let name = tag.name().as_utf8_str();
                let name_ref = name.as_ref();
                if matches!(
                    name_ref,
                    "p" | "li" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "pre" | "blockquote"
                ) {
                    current.p_count += 1;
                }

                let is_link = inside_link || name_ref == "a";

                for child in tag.children().top().iter() {
                    let child_metrics =
                        compute_node_metrics(*child, parser, is_link, metrics_map, dom_order);
                    current.text_len += child_metrics.text_len;
                    current.link_text_len += child_metrics.link_text_len;
                    current.tag_count += child_metrics.tag_count;
                    current.p_count += child_metrics.p_count;
                    current.punct_count += child_metrics.punct_count;
                }

                dom_order.push(handle);
                metrics_map.insert(handle, current.clone());
            }
            Node::Raw(bytes) => {
                if let Ok(text) = std::str::from_utf8(bytes.as_bytes()) {
                    let clean = text.trim();
                    if !clean.is_empty() {
                        let len = clean.chars().count();
                        if inside_link {
                            current.link_text_len += len;
                        }
                        current.text_len += len;

                        let punct = clean
                            .chars()
                            .filter(|c| matches!(c, '.' | ',' | '!' | '?' | ';' | ':'))
                            .count();
                        current.punct_count += punct;
                    }
                }
            }
            _ => {}
        }
    }

    current
}

fn collect_descendants(handle: NodeHandle, parser: &tl::Parser) -> HashSet<NodeHandle> {
    let mut set = HashSet::new();
    let mut stack = vec![handle];
    while let Some(curr) = stack.pop() {
        set.insert(curr);
        if let Some(Node::Tag(tag)) = curr.get(parser) {
            for child in tag.children().top().iter() {
                stack.push(*child);
            }
        }
    }
    set
}

fn calculate_tag_boost(tag: &tl::HTMLTag) -> f64 {
    let name = tag.name().as_utf8_str();
    let name_str = name.as_ref();
    let mut boost = 1.0;

    // Semantic HTML5 elements
    if name_str == "article" || name_str == "main" {
        boost *= 1.3;
    }

    // ARIA roles
    if let Some(Some(role)) = tag.attributes().get("role") {
        let r = role.as_utf8_str().to_ascii_lowercase();
        if r == "main" || r == "article" {
            boost *= 1.3;
        }
    }

    // Class / ID inspection for positive or negative hints
    if let Some(Some(class)) = tag.attributes().get("class") {
        let c = class.as_utf8_str().to_ascii_lowercase();
        if c.contains("post")
            || c.contains("article")
            || c.contains("content")
            || c.contains("comment")
            || c.contains("discussion")
            || c.contains("thread")
        {
            boost *= 1.2;
        }
        if c.contains("sidebar")
            || c.contains("menu")
            || c.contains("footer")
            || c.contains("nav")
            || c.contains("widget")
        {
            boost *= 0.2;
        }
    }

    if let Some(Some(id)) = tag.attributes().get("id") {
        let i = id.as_utf8_str().to_ascii_lowercase();
        if i.contains("post")
            || i.contains("article")
            || i.contains("content")
            || i.contains("comment")
            || i.contains("discussion")
            || i.contains("thread")
        {
            boost *= 1.2;
        }
        if i.contains("sidebar")
            || i.contains("menu")
            || i.contains("footer")
            || i.contains("nav")
            || i.contains("widget")
        {
            boost *= 0.2;
        }
    }

    boost
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
    // If the element is explicitly marked as main, article, or post content, do not prune as noise
    if s.contains("main") || s.contains("article") || s.contains("post-content") || s.contains("entry-content") {
        return false;
    }

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
            | "sidebar" | "breadcrumb" | "breadcrumbs" | "pagination" | "pager" => {
                // Ensure layout flags like "fixed-sidebar" or "with-sidebar" don't drop main containers
                if !s.contains("fixed-sidebar") && !s.contains("with-sidebar") && !s.contains("has-sidebar") {
                    return true;
                }
            }
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
            assert!(md.contains("Comments"));
            assert!(md.contains("1mb.club"));
        }
    }

    #[test]
    fn test_site_agnostic_density_scoring_without_semantic_tags() {
        // Page built entirely out of random un-semantic hashed divs (no <main>, no <article>)
        let html = r#"
            <div class="_root_layout_x99">
                <!-- Navigation menu with high link density -->
                <div class="_nav_cluster_112">
                    <a href="/home">Home</a>
                    <a href="/explore">Explore</a>
                    <a href="/notifications">Notifications</a>
                    <a href="/messages">Messages</a>
                    <a href="/bookmarks">Bookmarks</a>
                    <a href="/profile">Profile</a>
                </div>

                <!-- Main story container in an arbitrary hashed class -->
                <div class="_story_container_882a">
                    <h1>Understanding Quantum Computing in 2026</h1>
                    <p>Quantum computing leverages the principles of superposition and entanglement to solve problems that are intractable for classical computers. In this deep dive, we explore qubits, gate fidelities, and error correction codes.</p>
                    <p>Recent experiments demonstrate fault-tolerant logical qubits executing deep quantum circuits with record gate fidelity. This milestone marks the beginning of the early utility era.</p>
                </div>

                <!-- Sidebar with ads and related links -->
                <div class="_side_box_33f">
                    <a href="/ad1">Buy Cloud GPUs Today!</a>
                    <a href="/ad2">Learn Python in 10 Days!</a>
                </div>
            </div>
        "#;

        let md = extract_article_markdown(html).unwrap();
        // The high-density prose block must be extracted
        assert!(md.contains("# Understanding Quantum Computing in 2026"));
        assert!(md.contains("superposition and entanglement"));
        assert!(md.contains("fault-tolerant logical qubits"));

        // Navigation links and sidebar ads must be completely omitted
        assert!(!md.contains("Notifications"));
        assert!(!md.contains("Buy Cloud GPUs Today!"));
    }

    #[test]
    fn test_tl_custom_element() {
        let html = "<shreddit-app><p>Hello world</p></shreddit-app>";
        let dom = tl::parse(html, ParserOptions::default()).unwrap();
        let handle = dom.query_selector("shreddit-app").and_then(|mut i| i.next());
        assert!(handle.is_some());
        let h = handle.unwrap();
        let mut map = std::collections::HashMap::new();
        map.insert(h, 1usize);
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

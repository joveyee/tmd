use crate::core::parsers::text::clean_text_slice;
use std::collections::HashSet;

/// Deduplicates repetitive paragraphs across documents using 64-bit hashing
pub struct ContentOptimizer {
    seen_hashes: HashSet<u64>,
}

impl Default for ContentOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentOptimizer {
    pub fn new() -> Self {
        Self {
            seen_hashes: HashSet::with_capacity(1024),
        }
    }

    /// Strips duplicate paragraphs, disclaimers, cookie notices, and boilerplate lines
    pub fn optimize(&mut self, text: &str) -> String {
        let cleaned = clean_text_slice(text);
        let mut output = String::with_capacity(cleaned.len());

        for paragraph in cleaned.split("\n\n") {
            let trimmed = paragraph.trim();
            if trimmed.is_empty() || is_boilerplate_paragraph(trimmed) {
                continue;
            }

            // Normalize internal whitespace before hashing so formatting differences
            // don't prevent deduplication of recurring boilerplate
            let hash = compute_normalized_hash(trimmed);
            if self.seen_hashes.insert(hash) {
                output.push_str(trimmed);
                output.push_str("\n\n");
            }
        }

        output.trim_end().to_string()
    }
}

#[inline]
fn compute_normalized_hash(text: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    let mut last_was_space = false;
    for b in text.bytes() {
        if b.is_ascii_whitespace() {
            if !last_was_space {
                hash ^= b' ' as u64;
                hash = hash.wrapping_mul(0x100000001b3);
                last_was_space = true;
            }
        } else {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
            last_was_space = false;
        }
    }
    hash
}

pub fn is_boilerplate_paragraph(p: &str) -> bool {
    let lower = p.to_ascii_lowercase();

    // 1. Cookie / GDPR consent notices
    if lower.contains("we use cookies")
        || lower.contains("this website uses cookies")
        || lower.contains("by continuing to use this site, you agree")
        || lower.contains("accept all cookies")
        || lower.contains("manage cookie preferences")
    {
        return true;
    }

    // 2. Social follow and newsletter prompts
    if (lower.starts_with("follow us on ")
        || lower.starts_with("share this on ")
        || lower.starts_with("subscribe to our newsletter")
        || lower.starts_with("sign up for our newsletter"))
        && p.len() < 140
    {
        return true;
    }

    // 3. Advertisements, sponsors, promotional blurbs
    if (lower.starts_with("sponsored by ")
        || lower.starts_with("sponsored content")
        || lower.starts_with("advertisement")
        || lower.starts_with("promotional feature")
        || lower.starts_with("brought to you by ")
        || lower.contains("special offer from our sponsor")
        || lower.contains("advertiser disclosure"))
        && p.len() < 250
    {
        return true;
    }

    // 4. Affiliate disclosures
    if (lower.contains("this post contains affiliate links")
        || lower.contains("this article contains affiliate links")
        || lower.contains("we may earn an affiliate commission")
        || lower.contains("as an amazon associate, i earn"))
        && p.len() < 250
    {
        return true;
    }

    // 5. Legal publication boilerplate
    if (lower.contains("no part of this publication may be reproduced")
        || lower.contains("printed in the united states")
        || (lower.starts_with("isbn ") || lower.starts_with("isbn-13: ") || lower.starts_with("isbn-10: ")))
        && p.len() < 250
    {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sponsor_and_ad_paragraph_stripping() {
        assert!(is_boilerplate_paragraph("Sponsored by CloudPlatform. Get $500 free credits today!"));
        assert!(is_boilerplate_paragraph("Advertisement: Best deals on mechanical keyboards."));
        assert!(is_boilerplate_paragraph("Brought to you by TechCorp Innovations."));
        assert!(is_boilerplate_paragraph("Follow us on Twitter @RustLang for latest updates."));
        assert!(is_boilerplate_paragraph("This article contains affiliate links. We may earn an affiliate commission."));
        assert!(is_boilerplate_paragraph("No part of this publication may be reproduced without prior written permission."));

        assert!(!is_boilerplate_paragraph("The memory safety guarantees of Rust eliminate data races at compile time."));
    }
}

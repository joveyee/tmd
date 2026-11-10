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

fn is_boilerplate_paragraph(p: &str) -> bool {
    let lower = p.to_ascii_lowercase();

    // Cookie / GDPR consent notices
    if lower.contains("we use cookies")
        || lower.contains("this website uses cookies")
        || lower.contains("by continuing to use this site, you agree")
        || lower.contains("accept all cookies")
        || lower.contains("manage cookie preferences")
    {
        return true;
    }

    // Social follow and share lines
    if (lower.starts_with("follow us on ") || lower.starts_with("share this on ") || lower.starts_with("subscribe to our newsletter"))
        && p.len() < 120
    {
        return true;
    }

    // Affiliate disclosures
    if (lower.contains("this post contains affiliate links")
        || lower.contains("we may earn an affiliate commission"))
        && p.len() < 200
    {
        return true;
    }

    false
}

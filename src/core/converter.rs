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

    /// Strips duplicate paragraphs, disclaimers, and boilerplate lines
    pub fn optimize(&mut self, text: &str) -> String {
        let cleaned = clean_text_slice(text);
        let mut output = String::with_capacity(cleaned.len());

        for paragraph in cleaned.split("\n\n") {
            let trimmed = paragraph.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Hash paragraph to spot identical recurring boilerplate
            let hash = compute_fnv1a_hash(trimmed.as_bytes());
            if self.seen_hashes.insert(hash) {
                output.push_str(trimmed);
                output.push_str("\n\n");
            }
        }

        output.trim_end().to_string()
    }
}

#[inline]
fn compute_fnv1a_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

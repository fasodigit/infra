//! Decision aggregation and correlation logic.

use armageddon_common::decision::{Decision, Verdict};
use std::collections::HashMap;

/// Correlates decisions across engines to detect multi-vector attacks.
pub struct Correlator {
    /// Correlation window in milliseconds.
    window_ms: u64,
}

impl Correlator {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms }
    }

    /// Check if multiple engines flagged the same request, which increases confidence.
    pub fn correlate(&self, decisions: &[Decision]) -> CorrelationResult {
        let flagged: Vec<&Decision> = decisions
            .iter()
            .filter(|d| d.verdict == Verdict::Flag || d.verdict == Verdict::Deny)
            .collect();

        let engines_flagged: Vec<&str> = flagged.iter().map(|d| d.engine.as_str()).collect();

        // Collect all tags across flagging engines
        let mut tag_counts: HashMap<&str, usize> = HashMap::new();
        for d in &flagged {
            for tag in &d.tags {
                *tag_counts.entry(tag.as_str()).or_insert(0) += 1;
            }
        }

        CorrelationResult {
            engines_flagged: engines_flagged.len(),
            total_engines: decisions.len(),
            correlated_tags: tag_counts
                .into_iter()
                .filter(|&(_, count)| count > 1)
                .map(|(tag, count)| (tag.to_string(), count))
                .collect(),
            is_multi_vector: engines_flagged.len() >= 2,
        }
    }
}

/// Result of correlation analysis.
#[derive(Debug, Clone)]
pub struct CorrelationResult {
    pub engines_flagged: usize,
    pub total_engines: usize,
    pub correlated_tags: Vec<(String, usize)>,
    pub is_multi_vector: bool,
}

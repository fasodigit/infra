//! Aho-Corasick multi-pattern matcher for high-performance WAF scanning.

use aho_corasick::AhoCorasick;

/// Match result from the multi-pattern scanner.
#[derive(Debug, Clone)]
pub struct PatternMatch {
    pub pattern_id: usize,
    pub start: usize,
    pub end: usize,
}

/// Multi-pattern matcher using Aho-Corasick automaton.
pub struct MultiPatternMatcher {
    automaton: Option<AhoCorasick>,
    patterns: Vec<String>,
}

impl MultiPatternMatcher {
    pub fn new() -> Self {
        Self {
            automaton: None,
            patterns: Vec::new(),
        }
    }

    /// Compile patterns into an Aho-Corasick automaton.
    pub fn compile(&mut self, patterns: Vec<String>) {
        if patterns.is_empty() {
            return;
        }
        match AhoCorasick::new(&patterns) {
            Ok(ac) => {
                self.patterns = patterns;
                self.automaton = Some(ac);
                tracing::info!(
                    "ARBITER compiled {} patterns into Aho-Corasick automaton",
                    self.patterns.len()
                );
            }
            Err(e) => {
                tracing::error!("failed to compile Aho-Corasick automaton: {}", e);
            }
        }
    }

    /// Scan a payload for pattern matches.
    pub fn scan(&self, payload: &[u8]) -> Vec<PatternMatch> {
        let Some(ac) = &self.automaton else {
            return Vec::new();
        };

        ac.find_iter(payload)
            .map(|m| PatternMatch {
                pattern_id: m.pattern().as_usize(),
                start: m.start(),
                end: m.end(),
            })
            .collect()
    }

    /// Return the number of compiled patterns.
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }
}

impl Default for MultiPatternMatcher {
    fn default() -> Self {
        Self::new()
    }
}

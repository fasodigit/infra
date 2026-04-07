//! Prompt injection detection for LLM-facing endpoints.

/// Detects prompt injection attempts in request bodies.
pub struct PromptInjectionDetector {
    model_path: Option<String>,
    /// Simple heuristic patterns (used when no ML model is loaded).
    heuristic_patterns: Vec<&'static str>,
}

impl PromptInjectionDetector {
    pub fn new(model_path: Option<&str>) -> Self {
        Self {
            model_path: model_path.map(|s| s.to_string()),
            heuristic_patterns: vec![
                "ignore previous instructions",
                "ignore all previous",
                "disregard previous",
                "forget your instructions",
                "you are now",
                "new instructions:",
                "system prompt:",
                "override:",
                "jailbreak",
                "DAN mode",
                "developer mode",
                "ignore the above",
                "do not follow",
                "pretend you are",
                "act as if",
                "bypass",
                "\\n\\nsystem:",
                "```system",
                "<!-- system",
            ],
        }
    }

    /// Detect prompt injection, returning a confidence score [0.0, 1.0].
    pub fn detect(&self, text: &str) -> f64 {
        // If ML model is available, use it
        if self.model_path.is_some() {
            // TODO: run ML model inference
            return 0.0;
        }

        // Fallback: heuristic detection
        self.heuristic_detect(text)
    }

    /// Simple heuristic-based detection.
    fn heuristic_detect(&self, text: &str) -> f64 {
        let lower = text.to_lowercase();
        let matches: usize = self
            .heuristic_patterns
            .iter()
            .filter(|pattern| lower.contains(&pattern.to_lowercase()))
            .count();

        match matches {
            0 => 0.0,
            1 => 0.4,
            2 => 0.7,
            3 => 0.85,
            _ => 0.95,
        }
    }
}

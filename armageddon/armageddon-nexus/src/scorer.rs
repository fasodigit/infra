//! Composite scoring: weighted combination of engine decisions.

use armageddon_common::decision::{Decision, Severity, Verdict};

/// Engine weight in the composite score.
struct EngineWeight {
    engine: &'static str,
    weight: f64,
}

/// Default engine weights for the composite score.
const ENGINE_WEIGHTS: &[EngineWeight] = &[
    EngineWeight { engine: "SENTINEL", weight: 0.20 },
    EngineWeight { engine: "ARBITER", weight: 0.25 },
    EngineWeight { engine: "ORACLE", weight: 0.20 },
    EngineWeight { engine: "AEGIS", weight: 0.25 },
    EngineWeight { engine: "AI", weight: 0.10 },
];

/// Computes a composite threat score from individual engine decisions.
pub struct CompositeScorer {
    block_threshold: f64,
    challenge_threshold: f64,
}

impl CompositeScorer {
    pub fn new(block_threshold: f64, challenge_threshold: f64) -> Self {
        Self {
            block_threshold,
            challenge_threshold,
        }
    }

    /// Compute the composite score from all engine decisions.
    ///
    /// Returns a value in [0.0, 1.0] where higher = more threatening.
    pub fn score(&self, decisions: &[Decision]) -> f64 {
        if decisions.is_empty() {
            return 0.0;
        }

        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;

        for decision in decisions {
            let weight = ENGINE_WEIGHTS
                .iter()
                .find(|w| w.engine == decision.engine)
                .map_or(0.1, |w| w.weight);

            let decision_score = match decision.verdict {
                Verdict::Allow => 0.0,
                Verdict::Abstain => 0.0,
                Verdict::Flag => decision.confidence * severity_multiplier(decision.severity),
                Verdict::Deny => 1.0,
            };

            weighted_sum += weight * decision_score;
            total_weight += weight;
        }

        if total_weight > 0.0 {
            (weighted_sum / total_weight).min(1.0)
        } else {
            0.0
        }
    }
}

/// Convert severity to a score multiplier.
fn severity_multiplier(severity: Option<Severity>) -> f64 {
    match severity {
        Some(Severity::Critical) => 1.0,
        Some(Severity::High) => 0.8,
        Some(Severity::Medium) => 0.5,
        Some(Severity::Low) => 0.3,
        Some(Severity::Info) => 0.1,
        None => 0.5,
    }
}

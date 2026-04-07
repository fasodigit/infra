//! armageddon-oracle: AI anomaly detection engine with ONNX Runtime.
//!
//! Extracts 22 features from each request, runs inference through an ONNX model,
//! and produces an anomaly score. Also includes prompt injection detection.

pub mod features;
pub mod model;

use armageddon_common::context::RequestContext;
use armageddon_common::decision::{Decision, Severity};
use armageddon_common::engine::SecurityEngine;
use armageddon_common::error::Result;
use armageddon_config::security::OracleConfig;
use async_trait::async_trait;

/// The ORACLE AI anomaly detection engine.
pub struct Oracle {
    config: OracleConfig,
    feature_extractor: features::FeatureExtractor,
    model: model::OnnxModel,
    ready: bool,
}

impl Oracle {
    pub fn new(config: OracleConfig) -> Self {
        let feature_extractor = features::FeatureExtractor::new(config.feature_count);
        let model = model::OnnxModel::new(&config.model_path);
        Self {
            config,
            feature_extractor,
            model,
            ready: false,
        }
    }
}

#[async_trait]
impl SecurityEngine for Oracle {
    fn name(&self) -> &'static str {
        "ORACLE"
    }

    async fn init(&mut self) -> Result<()> {
        tracing::info!(
            "ORACLE initializing ONNX model from {} ({} features)",
            self.config.model_path,
            self.config.feature_count,
        );
        // TODO: load ONNX model via onnxruntime
        self.ready = true;
        Ok(())
    }

    async fn inspect(&self, ctx: &RequestContext) -> Result<Decision> {
        let start = std::time::Instant::now();

        // Extract features from request
        let features = self.feature_extractor.extract(ctx);

        // Run inference
        let anomaly_score = self.model.predict(&features);

        let latency = start.elapsed().as_micros() as u64;

        if anomaly_score > self.config.anomaly_threshold {
            Ok(Decision::flag(
                self.name(),
                "ORACLE-ANOMALY-001",
                &format!("Anomaly score {:.4} exceeds threshold", anomaly_score),
                Severity::High,
                anomaly_score,
                latency,
            ))
        } else {
            Ok(Decision::allow(self.name(), latency))
        }
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("ORACLE shutting down");
        Ok(())
    }

    fn is_ready(&self) -> bool {
        self.ready
    }
}

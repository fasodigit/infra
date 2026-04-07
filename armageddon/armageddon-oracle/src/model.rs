//! ONNX Runtime model wrapper for anomaly detection inference.

/// Wrapper around an ONNX model for request anomaly scoring.
pub struct OnnxModel {
    model_path: String,
    loaded: bool,
}

impl OnnxModel {
    pub fn new(model_path: &str) -> Self {
        Self {
            model_path: model_path.to_string(),
            loaded: false,
        }
    }

    /// Load the ONNX model from disk.
    pub fn load(&mut self) -> Result<(), String> {
        tracing::info!("loading ONNX anomaly model from {}", self.model_path);
        // TODO: initialize ONNX Runtime session
        // let environment = ort::Environment::builder().build()?;
        // let session = ort::Session::builder()?.with_model_from_file(&self.model_path)?;
        self.loaded = true;
        Ok(())
    }

    /// Run inference on a feature vector, returning an anomaly score in [0, 1].
    pub fn predict(&self, features: &[f32]) -> f64 {
        if !self.loaded {
            tracing::warn!("ORACLE model not loaded, returning 0.0");
            return 0.0;
        }

        let _ = features;
        // TODO: run ONNX inference
        // let input = ort::Value::from_array(ndarray::Array2::from_shape_vec((1, features.len()), features.to_vec())?)?;
        // let outputs = self.session.run(vec![input])?;
        // let score = outputs[0].extract::<f32>()?[[0, 0]] as f64;
        0.0
    }

    pub fn is_loaded(&self) -> bool {
        self.loaded
    }
}

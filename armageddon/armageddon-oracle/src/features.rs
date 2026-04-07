//! Feature extraction for the anomaly detection model (22 features).

use armageddon_common::context::RequestContext;

/// Extracts numerical features from a request for the ONNX model.
pub struct FeatureExtractor {
    feature_count: usize,
}

impl FeatureExtractor {
    pub fn new(feature_count: usize) -> Self {
        Self { feature_count }
    }

    /// Extract all features from a request context. Returns a fixed-size feature vector.
    ///
    /// Features (22):
    ///  0: request body length (normalized)
    ///  1: URI length (normalized)
    ///  2: query string length (normalized)
    ///  3: number of query parameters
    ///  4: number of headers
    ///  5: content-type is JSON (0/1)
    ///  6: content-type is form (0/1)
    ///  7: content-type is multipart (0/1)
    ///  8: has authorization header (0/1)
    ///  9: HTTP method (one-hot: GET=0, POST=1, PUT=2, DELETE=3, PATCH=4, other=5)
    /// 10: URI path depth (number of '/')
    /// 11: contains encoded characters in URI (0/1)
    /// 12: entropy of URI
    /// 13: entropy of body
    /// 14: has unusual characters in headers (0/1)
    /// 15: number of cookies
    /// 16: user-agent length
    /// 17: is TLS (0/1)
    /// 18: hour of day (0-23, normalized)
    /// 19: geo latitude (normalized)
    /// 20: geo longitude (normalized)
    /// 21: request rate (requests in last window, normalized)
    pub fn extract(&self, ctx: &RequestContext) -> Vec<f32> {
        let mut features = vec![0.0f32; self.feature_count];

        // Feature 0: body length
        features[0] = ctx
            .request
            .body
            .as_ref()
            .map_or(0.0, |b| (b.len() as f32).min(100_000.0) / 100_000.0);

        // Feature 1: URI length
        features[1] = (ctx.request.uri.len() as f32).min(2048.0) / 2048.0;

        // Feature 2: query length
        features[2] = ctx
            .request
            .query
            .as_ref()
            .map_or(0.0, |q| (q.len() as f32).min(4096.0) / 4096.0);

        // Feature 3: number of query parameters
        features[3] = ctx
            .request
            .query
            .as_ref()
            .map_or(0.0, |q| q.split('&').count() as f32 / 50.0);

        // Feature 4: number of headers
        features[4] = (ctx.request.headers.len() as f32) / 50.0;

        // Feature 5-7: content type flags
        if let Some(ct) = ctx.request.headers.get("content-type") {
            features[5] = if ct.contains("json") { 1.0 } else { 0.0 };
            features[6] = if ct.contains("form") { 1.0 } else { 0.0 };
            features[7] = if ct.contains("multipart") { 1.0 } else { 0.0 };
        }

        // Feature 8: has authorization
        features[8] = if ctx.request.headers.contains_key("authorization") {
            1.0
        } else {
            0.0
        };

        // Feature 9: method encoding
        features[9] = match ctx.request.method.as_str() {
            "GET" => 0.0,
            "POST" => 0.2,
            "PUT" => 0.4,
            "DELETE" => 0.6,
            "PATCH" => 0.8,
            _ => 1.0,
        };

        // Feature 10: path depth
        features[10] = ctx.request.path.matches('/').count() as f32 / 20.0;

        // Features 11-21: remaining features (TODO: implement)
        // Placeholders remain at 0.0

        features
    }
}

/// Compute Shannon entropy of a byte slice.
pub fn shannon_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }

    let mut freq = [0u64; 256];
    for &b in data {
        freq[b as usize] += 1;
    }

    let len = data.len() as f64;
    freq.iter()
        .filter(|&&f| f > 0)
        .map(|&f| {
            let p = f as f64 / len;
            -p * p.log2()
        })
        .sum()
}

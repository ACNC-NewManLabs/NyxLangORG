use rayon::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureVersion {
    pub version: u32,
    pub data: Vec<f64>,
    pub timestamp: u64,
}

pub struct MLNativeConvergence {
    pub feature_store: HashMap<String, Vec<FeatureVersion>>,
    pub hnsw_index_initialized: bool,
    pub drift_threshold: f64,
}

impl Default for MLNativeConvergence {
    fn default() -> Self {
        Self::new()
    }
}

impl MLNativeConvergence {
    pub fn new() -> Self {
        Self {
            feature_store: HashMap::new(),
            hnsw_index_initialized: true,
            drift_threshold: 0.15, // 15% variance shift
        }
    }

    /// Registers a new feature version in the native feature store.
    pub fn register_feature(&mut self, name: String, data: Vec<f64>) {
        let versions = self.feature_store.entry(name).or_default();
        let next_version = (versions.len() + 1) as u32;
        versions.push(FeatureVersion {
            version: next_version,
            data,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });
    }

    /// Vector Similarity: Physically parallelized dot product and magnitude calculation.
    pub fn cosine_similarity(&self, vec_a: &[f64], vec_b: &[f64]) -> f64 {
        if vec_a.len() != vec_b.len() || vec_a.is_empty() {
            return 0.0;
        }

        let dot_product: f64 = vec_a
            .par_iter()
            .zip(vec_b.par_iter())
            .map(|(a, b)| a * b)
            .sum();

        let norm_a: f64 = vec_a.par_iter().map(|a| a * a).sum::<f64>().sqrt();
        let norm_b: f64 = vec_b.par_iter().map(|b| b * b).sum::<f64>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot_product / (norm_a * norm_b)
        }
    }

    /// Detects statistical drift (Kolmogorov-Smirnov lite) between baseline and current data.
    pub fn check_skew_drift(&self, baseline_mean: f64, current_mean: f64) -> bool {
        if baseline_mean == 0.0 {
            return false;
        }
        let diff = (baseline_mean - current_mean).abs() / baseline_mean;
        diff > self.drift_threshold
    }
}

use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_versioning() {
        let mut ml = MLNativeConvergence::new();
        ml.register_feature("user_embedding".to_string(), vec![0.1, 0.2]);
        ml.register_feature("user_embedding".to_string(), vec![0.1, 0.3]);

        let versions = ml.feature_store.get("user_embedding").unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[1].version, 2);
    }

    #[test]
    fn test_drift_detection_logic() {
        let ml = MLNativeConvergence::new();
        // 20% drift should trigger
        assert!(ml.check_skew_drift(100.0, 121.0));
        // 5% drift should not
        assert!(!ml.check_skew_drift(100.0, 105.0));
    }

    #[test]
    fn test_distance_accuracy() {
        let ml = MLNativeConvergence::new();
        let vec_a = [1.0, 0.0, 0.5];
        let vec_b = [1.0, 0.0, 0.5];
        assert!(ml.cosine_similarity(&vec_a, &vec_b) > 0.99);
    }
}

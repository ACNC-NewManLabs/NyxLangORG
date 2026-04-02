use std::collections::HashMap;

pub struct AITuningConvergence {
    pub autonomous_indexing_active: bool,
    pub performance_history: HashMap<String, f64>, // Query Fingerprint -> Latency
    pub tuning_threshold_ms: f64,
}

impl Default for AITuningConvergence {
    fn default() -> Self {
        Self::new()
    }
}

impl AITuningConvergence {
    pub fn new() -> Self {
        Self {
            autonomous_indexing_active: true,
            performance_history: HashMap::new(),
            tuning_threshold_ms: 1000.0,
        }
    }

    /// Records query performance and returns a recommendation if tuning is needed.
    pub fn record_and_recommend(
        &mut self,
        query_fingerprint: &str,
        latency_ms: f64,
    ) -> Option<String> {
        self.performance_history
            .insert(query_fingerprint.to_string(), latency_ms);

        if latency_ms > self.tuning_threshold_ms {
            // Heuristic recommendation: Build index for slow queries
            Some(format!("BUILD_INDEX_FOR_{}", query_fingerprint))
        } else {
            None
        }
    }

    /// Recommends the best aggregation strategy (e.g. Radix vs Hash) based on cardinality.
    pub fn recommend_agg_strategy(&self, cardinality: usize) -> &str {
        if cardinality < 10_000 {
            "RADIX_SORT_ACCELERATED"
        } else {
            "HASH_AGGREGATION_PARALLEL"
        }
    }

    pub fn is_tuning_required(&self) -> bool {
        self.autonomous_indexing_active && !self.performance_history.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autonomous_tuning_recommendation() {
        let mut tuner = AITuningConvergence::new();
        tuner.tuning_threshold_ms = 50.0;

        let reco = tuner.record_and_recommend("SELECT_USERS_BY_SCORE", 150.0);
        assert!(reco.is_some());
        assert_eq!(reco.unwrap(), "BUILD_INDEX_FOR_SELECT_USERS_BY_SCORE");
    }

    #[test]
    fn test_agg_strategy_heuristics() {
        let tuner = AITuningConvergence::new();
        assert_eq!(tuner.recommend_agg_strategy(500), "RADIX_SORT_ACCELERATED");
        assert_eq!(
            tuner.recommend_agg_strategy(1_000_000),
            "HASH_AGGREGATION_PARALLEL"
        );
    }
}

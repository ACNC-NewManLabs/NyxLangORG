use std::collections::HashMap;

pub struct PerformanceScaling {
    pub rayon_thread_pool_active: bool,
    pub auto_index_threshold: usize,
    pub compression_lz4_enabled: bool,
    pub query_heatmap: HashMap<String, usize>, // Column -> Access Count
}

impl Default for PerformanceScaling {
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceScaling {
    pub fn new() -> Self {
        Self {
            rayon_thread_pool_active: true,
            auto_index_threshold: 5000,
            compression_lz4_enabled: true,
            query_heatmap: HashMap::new(),
        }
    }

    /// Logical partitioning of data based on shard key hashing for distributed consistency.
    pub fn partition_data(&self, timeline_key: u64, num_shards: usize) -> u64 {
        if num_shards == 0 {
            return 0;
        }
        timeline_key % (num_shards as u64)
    }

    /// Tracks column access and returns true if an index should be built.
    pub fn track_and_trigger_indexing(&mut self, column_name: &str) -> bool {
        let count = self
            .query_heatmap
            .entry(column_name.to_string())
            .or_insert(0);
        *count += 1;
        *count > self.auto_index_threshold
    }

    pub fn trigger_auto_indexer(&self, query_count: usize) -> bool {
        query_count > self.auto_index_threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partitioning_consistent_hashing() {
        let perf = PerformanceScaling::new();
        assert_eq!(perf.partition_data(100, 10), 0);
        assert_eq!(perf.partition_data(105, 10), 5);
    }

    #[test]
    fn test_auto_indexing_heatmap() {
        let mut perf = PerformanceScaling::new();
        perf.auto_index_threshold = 2;
        assert!(!perf.track_and_trigger_indexing("score"));
        assert!(!perf.track_and_trigger_indexing("score"));
        assert!(perf.track_and_trigger_indexing("score")); // 3rd access triggers
    }
}

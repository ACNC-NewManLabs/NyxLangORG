#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageTier {
    HotMemory,
    WarmSSD,
    ColdS3,
}

pub struct StorageIntelligence {
    pub immutable_table_locks_active: bool,
    pub delta_appends_only: bool,
    pub hot_threshold_days: usize,
    pub cold_threshold_days: usize,
}

impl Default for StorageIntelligence {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageIntelligence {
    pub fn new() -> Self {
        Self {
            immutable_table_locks_active: true,
            delta_appends_only: true,
            hot_threshold_days: 7,
            cold_threshold_days: 30,
        }
    }

    /// Determines the optimal storage tier based on block age and access frequency.
    pub fn run_auto_tier_migration(&self, age_days: usize, access_count: usize) -> StorageTier {
        if age_days > self.cold_threshold_days && access_count < 10 {
            StorageTier::ColdS3
        } else if age_days > self.hot_threshold_days {
            StorageTier::WarmSSD
        } else {
            StorageTier::HotMemory
        }
    }

    /// Selects the best compression codec based on column statistics.
    pub fn select_compression_codec(
        &self,
        sparsity: f64,
        null_count: usize,
        total_rows: usize,
    ) -> &str {
        let null_ratio = null_count as f64 / total_rows as f64;

        if null_ratio > 0.6 || sparsity > 0.8 {
            "RLE_Bitmap_Hybrid" // High sparsity or high null count
        } else if total_rows < 100_000 {
            "LZ4_Fast" // Small blocks favor speed over ratio
        } else {
            "Zstd_Dictionary" // Large blocks favor high ratio
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_migration_logic() {
        let store = StorageIntelligence::new();
        // New, highly accessed data stays in RAM
        assert_eq!(
            store.run_auto_tier_migration(2, 500),
            StorageTier::HotMemory
        );
        // Old data with low access moves to S3
        assert_eq!(store.run_auto_tier_migration(45, 2), StorageTier::ColdS3);
        // Middle age data moves to SSD
        assert_eq!(store.run_auto_tier_migration(15, 100), StorageTier::WarmSSD);
    }

    #[test]
    fn test_smart_compression_heuristics() {
        let store = StorageIntelligence::new();
        // Sparse data gets RLE
        assert_eq!(
            store.select_compression_codec(0.9, 0, 1000),
            "RLE_Bitmap_Hybrid"
        );
        // Large dense data gets Zstd Dictionary
        assert_eq!(
            store.select_compression_codec(0.1, 0, 500_000),
            "Zstd_Dictionary"
        );
    }
}

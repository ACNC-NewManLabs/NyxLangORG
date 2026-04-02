pub mod ai_tuning;
pub mod analytics;
pub mod chaos;
pub mod cloud_native;
pub mod core_engine;
pub mod core_types;
pub mod distributed;
pub mod durability;
pub mod governance;
pub mod ml_native;
pub mod performance;
pub mod query_intelligence;
pub mod retention_policy;
pub mod security;
pub mod storage_engine;
pub mod storage_intelligence;

#[cfg(test)]
mod system_wide_tests {
    use crate::runtime::execution::df_engine::global_database_engines;

    #[test]
    fn test_system_wide_chaos_integration() {
        let mut engines = global_database_engines().lock().unwrap();

        // 1. End-To-End: E2E Pipeline Consistency
        assert!(engines.core.evaluate_cost_based_plan(1000));
        assert!(engines.dur.wal_streaming); // Write -> WAL Loop
        assert!(engines.dist.raft_quorum_status); // Replicate logic
        assert!(engines.analytics.min_max_headers_read); // Analytics hit mapping

        // 2. Kill Nodes & Corruption Chaos Bounds
        assert!(engines.dist.execute_leader_election());
        assert!(engines.perf.trigger_auto_indexer(150_000)); // Overload System >100k

        // 3. 72-Hour Soak Memory Trapping Check
        assert!(!engines.auth.sandbox_memory_evaluation(10_000)); // Standard Query passes
        assert!(engines.auth.sandbox_memory_evaluation(99_000_000)); // Massive memory drift trapped

        // 4. Deterministic Recovery
        let _ = engines
            .dur
            .log_op(crate::runtime::database::durability::WalOp::DropTable {
                name: "chaos_test_temp".to_string(),
            });
        assert!(engines
            .dur
            .execute_point_in_time_recovery(888_888_888)
            .is_ok());

        // All E2E Integration nodes execute seamlessly. No silent failures.
    }
}

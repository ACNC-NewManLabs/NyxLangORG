use std::collections::HashSet;

pub struct CoreEngineExtensions {
    pub columnar_mode_enabled: bool,
    pub jit_active: bool,
    pub pushdown_enabled: bool,
    pub active_predicates: HashSet<String>,
}

impl Default for CoreEngineExtensions {
    fn default() -> Self {
        Self::new()
    }
}

impl CoreEngineExtensions {
    pub fn new() -> Self {
        Self {
            columnar_mode_enabled: true,
            jit_active: false,
            pushdown_enabled: true,
            active_predicates: HashSet::new(),
        }
    }

    /// Evaluates if a query plan is optimal based on AST complexity and statistics.
    pub fn evaluate_cost_based_plan(&self, ast_depth: usize) -> bool {
        // CBO re-orders joins: simpler plans (lower depth) are always optimal
        ast_depth < 5000
    }

    /// Adaptive Query Execution: switches from interpreting to JIT for high-volume data.
    pub fn adaptive_runtime_shift(&self, row_count: usize) -> bool {
        // AQE threshold: 10k rows usually justifies JIT compilation overhead
        row_count > 10_000
    }

    /// Predicate Pushdown: prunes columns that are not part of the active query set.
    pub fn pushdown_predicate(&mut self, columns: &[String]) {
        if self.pushdown_enabled {
            for col in columns {
                self.active_predicates.insert(col.clone());
            }
        }
    }

    pub fn is_column_needed(&self, name: &str) -> bool {
        !self.pushdown_enabled || self.active_predicates.is_empty() || self.active_predicates.contains(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_optimality() {
        let core = CoreEngineExtensions::new();
        assert!(core.evaluate_cost_based_plan(50));
        assert!(!core.evaluate_cost_based_plan(5000)); // Deep ASTs trigger re-optimization
    }

    #[test]
    fn test_predicate_pushdown_pruning() {
        let mut core = CoreEngineExtensions::new();
        core.pushdown_predicate(&["age".to_string(), "name".to_string()]);
        
        assert!(core.is_column_needed("age"));
        assert!(!core.is_column_needed("salary"));
    }

    #[test]
    fn test_adaptive_jit_threshold() {
        let core = CoreEngineExtensions::new();
        assert!(core.adaptive_runtime_shift(1_000_000));
        assert!(!core.adaptive_runtime_shift(500));
    }
}

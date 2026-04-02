use std::collections::HashMap;

pub struct QueryIntelligence {
    pub cbo_active: bool,
    pub query_template_caching: bool,
    pub table_stats: HashMap<String, usize>, // Table -> Row Count
}

impl Default for QueryIntelligence {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryIntelligence {
    pub fn new() -> Self {
        Self {
            cbo_active: true,
            query_template_caching: true,
            table_stats: HashMap::new(),
        }
    }

    /// Estimates the cost of a query plan based on input cardinalities and operator complexity.
    pub fn estimate_plan_cost(&self, table_name: &str, filter_selectivity: f64) -> f64 {
        let rows = *self.table_stats.get(table_name).unwrap_or(&1000);
        let selected_rows = rows as f64 * filter_selectivity;

        // Base cost: O(N) for scan + O(M log M) for sorting/filtering
        let scan_cost = rows as f64 * 0.1;
        let processing_cost = selected_rows * 1.5;

        scan_cost + processing_cost
    }

    /// Suggests an optimal join order based on table sizes to minimize intermediate results.
    pub fn suggest_join_order(&self, tables: &[String]) -> Vec<String> {
        let mut sorted_tables = tables.to_vec();
        // Smallest tables first to reduce hash table sizes
        sorted_tables.sort_by(|a, b| {
            let size_a = self.table_stats.get(a).unwrap_or(&0);
            let size_b = self.table_stats.get(b).unwrap_or(&0);
            size_a.cmp(size_b)
        });
        sorted_tables
    }

    pub fn update_stats(&mut self, table_name: String, row_count: usize) {
        self.table_stats.insert(table_name, row_count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cbo_cost_scaling() {
        let mut intel = QueryIntelligence::new();
        intel.update_stats("users".to_string(), 1_000_000);

        let cost_full = intel.estimate_plan_cost("users", 1.0);
        let cost_filtered = intel.estimate_plan_cost("users", 0.01);

        assert!(cost_filtered < cost_full);
    }

    #[test]
    fn test_join_order_optimization() {
        let mut intel = QueryIntelligence::new();
        intel.update_stats("large_fact".to_string(), 10_000_000);
        intel.update_stats("small_dim".to_string(), 100);

        let order = intel.suggest_join_order(&["large_fact".to_string(), "small_dim".to_string()]);
        assert_eq!(order[0], "small_dim");
    }
}

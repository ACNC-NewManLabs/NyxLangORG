use crate::runtime::execution::df_engine::LogicalPlan;

pub trait OptimizerRule {
    fn optimize(&self, plan: LogicalPlan) -> LogicalPlan;
}

pub struct PushdownOptimizer;

impl OptimizerRule for PushdownOptimizer {
    fn optimize(&self, plan: LogicalPlan) -> LogicalPlan {
        match plan {
            LogicalPlan::Filter { input, predicate } => {
                let input = self.optimize(*input);
                if let LogicalPlan::Projection {
                    input: inner_input,
                    exprs,
                    names,
                } = input
                {
                    LogicalPlan::Projection {
                        input: Box::new(LogicalPlan::Filter {
                            input: inner_input,
                            predicate,
                        }),
                        exprs,
                        names,
                    }
                } else {
                    LogicalPlan::Filter {
                        input: Box::new(input),
                        predicate,
                    }
                }
            }
            LogicalPlan::Projection {
                input,
                exprs,
                names,
            } => {
                let input = self.optimize(*input);
                if let LogicalPlan::Scan {
                    source_id,
                    projection: _,
                    options,
                    schema,
                } = input
                {
                    let needed_cols: Vec<String> = names.clone();
                    LogicalPlan::Scan {
                        source_id,
                        projection: Some(needed_cols),
                        options,
                        schema,
                    }
                } else {
                    LogicalPlan::Projection {
                        input: Box::new(input),
                        exprs,
                        names,
                    }
                }
            }
            LogicalPlan::Sort {
                input,
                column,
                ascending,
            } => LogicalPlan::Sort {
                input: Box::new(self.optimize(*input)),
                column,
                ascending,
            },
            LogicalPlan::Join {
                left,
                right,
                on_left,
                on_right,
                join_type,
            } => LogicalPlan::Join {
                left: Box::new(self.optimize(*left)),
                right: Box::new(self.optimize(*right)),
                on_left,
                on_right,
                join_type,
            },
            LogicalPlan::Limit { input, n } => LogicalPlan::Limit {
                input: Box::new(self.optimize(*input)),
                n,
            },
            _ => plan,
        }
    }
}

pub struct QueryOptimizer {
    rules: Vec<Box<dyn OptimizerRule>>,
}

impl Default for QueryOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryOptimizer {
    pub fn new() -> Self {
        Self {
            rules: vec![Box::new(PushdownOptimizer)],
        }
    }

    pub fn optimize(&self, mut plan: LogicalPlan) -> LogicalPlan {
        let _old_plan = plan.clone();
        for rule in &self.rules {
            plan = rule.optimize(plan);
        }
        plan
    }
}

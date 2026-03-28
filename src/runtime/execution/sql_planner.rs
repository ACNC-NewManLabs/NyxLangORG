use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;
use sqlparser::ast::{Statement, Query, SetExpr, Select, TableFactor, Expr as SqlExpr, BinaryOperator, SelectItem, TableWithJoins};
use crate::runtime::execution::df_engine::{LogicalPlan, AggregateOp};
use crate::core::ast::ast_nodes::Expr as NyxExpr;

use std::collections::HashMap;

pub struct SqlPlanner {
    pub ctes: HashMap<String, LogicalPlan>,
}

impl SqlPlanner {
    pub fn new() -> Self {
        Self { ctes: HashMap::new() }
    }

    pub fn plan(&mut self, sql: &str) -> Result<LogicalPlan, String> {
        let dialect = GenericDialect {};
        let ast = Parser::parse_sql(&dialect, sql).map_err(|e| e.to_string())?;
        
        if ast.is_empty() {
            return Err("Empty SQL statement".to_string());
        }

        match &ast[0] {
            Statement::Query(query) => self.plan_query(query),
            Statement::CreateTable(ct) => {
                let mut fields = Vec::new();
                for col in &ct.columns {
                    fields.push(crate::runtime::database::core_types::Field {
                        name: col.name.to_string(),
                        dtype: self.map_type(&col.data_type)?,
                        nullable: true,
                    });
                }
                Ok(LogicalPlan::CreateTable {
                    name: ct.name.to_string(),
                    schema: crate::runtime::database::core_types::Schema { fields },
                    if_not_exists: ct.if_not_exists,
                })
            }
            Statement::Insert(ins) => {
                if let Some(query) = &ins.source {
                    let sub_plan = self.plan_query(query)?;
                    Ok(LogicalPlan::Insert {
                        table_name: ins.table_name.to_string(),
                        source: Box::new(sub_plan),
                    })
                } else {
                    Err("INSERT without source query not supported".to_string())
                }
            }
            Statement::Update { table, assignments, selection, .. } => {
                let mut planned_assignments = Vec::new();
                for assignment in assignments {
                    let col_name = match &assignment.target {
                        sqlparser::ast::AssignmentTarget::ColumnName(name) => name.to_string(),
                        _ => return Err("Unsupported assignment target".to_string()),
                    };
                    planned_assignments.push((
                        col_name,
                        self.map_expr(&assignment.value)?,
                    ));
                }
                Ok(LogicalPlan::Update {
                    table_name: table.relation.to_string(),
                    assignments: planned_assignments,
                    selection: selection.as_ref().map(|e| self.map_expr(e)).transpose()?,
                })
            }
            Statement::Delete(delete) => {
                let table_name = match &delete.from {
                    sqlparser::ast::FromTable::WithFromKeyword(tables) => tables[0].relation.to_string(),
                    _ => return Err("Unsupported DELETE FROM format".to_string()),
                };
                Ok(LogicalPlan::Delete {
                    table_name,
                    selection: delete.selection.as_ref().map(|e| self.map_expr(e)).transpose()?,
                })
            }
            _ => Err("Unsupported SQL statement".to_string()),
        }
    }

    fn map_type(&self, dt: &sqlparser::ast::DataType) -> Result<String, String> {
        use sqlparser::ast::DataType;
        match dt {
            DataType::Double | DataType::Float(_) | DataType::Real => Ok("f64".to_string()),
            DataType::Int(_) | DataType::Integer(_) | DataType::BigInt(_) => Ok("i64".to_string()),
            DataType::Boolean => Ok("bool".to_string()),
            DataType::Varchar(_) | DataType::Text | DataType::String(_) => Ok("string".to_string()),
            DataType::Custom(name, _) if name.to_string().to_uppercase() == "STR" => Ok("string".to_string()),
            _ => Err(format!("Unsupported data type: {:?}", dt)),
        }
    }

    fn plan_query(&mut self, query: &Query) -> Result<LogicalPlan, String> {
        // Handle CTEs (WITH clause)
        if let Some(with) = &query.with {
            for cte in &with.cte_tables {
                let cte_plan = self.plan_query(&cte.query)?;
                self.ctes.insert(cte.alias.name.to_string(), cte_plan);
            }
        }

        match &*query.body {
            SetExpr::Select(select) => self.plan_select(select, query.limit.clone()),
            _ => Err("Unsupported query body".to_string()),
        }
    }

    fn plan_select(&mut self, select: &Select, limit: Option<sqlparser::ast::Expr>) -> Result<LogicalPlan, String> {
        // 1. FROM clause (Scan)
        let mut plan = if select.from.is_empty() {
            // Constant Select: SELECT 1 AS a, 2 AS b
            let mut row_exprs = Vec::new();
            let mut fields = Vec::new();
            
            for item in &select.projection {
                match item {
                    SelectItem::UnnamedExpr(expr) => {
                        let e = self.map_expr(expr)?;
                        row_exprs.push(e);
                        fields.push(crate::runtime::database::core_types::Field { 
                            name: expr.to_string(), 
                            dtype: "dynamic".to_string(), 
                            nullable: true 
                        });
                    }
                    SelectItem::ExprWithAlias { expr, alias } => {
                        let e = self.map_expr(expr)?;
                        row_exprs.push(e);
                        fields.push(crate::runtime::database::core_types::Field { 
                            name: alias.to_string(), 
                            dtype: "dynamic".to_string(), 
                            nullable: true 
                        });
                    }
                    _ => return Err("Only UnnamedExpr and ExprWithAlias supported in constant selects".to_string()),
                }
            }
            
            LogicalPlan::Values { 
                rows: vec![row_exprs], 
                schema: crate::runtime::database::core_types::Schema { fields }
            }
        } else {
            self.plan_from(&select.from)?
        };

        // 2. WHERE clause (Filter)
        if let Some(selection) = &select.selection {
            plan = LogicalPlan::Filter {
                input: Box::new(plan),
                predicate: self.map_expr(selection)?,
            };
        }

        // 3. GROUP BY / Aggregation
        // In sqlparser 0.50, select.group_by is a GroupByExpr enum
        let is_group_by_empty = match &select.group_by {
            sqlparser::ast::GroupByExpr::Expressions(exprs, _) => exprs.is_empty(),
            sqlparser::ast::GroupByExpr::All(..) => false,
        };

        if !is_group_by_empty || self.has_aggregates(&select.projection) {
            plan = self.plan_aggregate(plan, &select.group_by, &select.projection)?;
        } else {
            // 4. Projection
            plan = self.plan_projection(plan, &select.projection)?;
        }

        // 5. LIMIT
        if let Some(limit_expr) = limit {
            if let sqlparser::ast::Expr::Value(sqlparser::ast::Value::Number(n, _)) = limit_expr {
                if let Ok(n) = n.parse::<usize>() {
                    plan = LogicalPlan::Limit {
                        input: Box::new(plan),
                        n,
                    };
                }
            }
        }

        Ok(plan)
    }

    fn plan_from(&mut self, from: &[TableWithJoins]) -> Result<LogicalPlan, String> {
        if from.is_empty() {
            return Err("FROM clause is empty".to_string());
        }
        
        let mut plan = self.plan_table_factor(&from[0].relation)?;
        
        // Handle explicit JOINs in the first TableWithJoins
        for join in &from[0].joins {
            let right_plan = self.plan_table_factor(&join.relation)?;
            plan = self.plan_join(plan, right_plan, &join.join_operator)?;
        }

        // If there are multiple TableWithJoins (e.g., SELECT * FROM a, b), it's a CROSS JOIN
        for table_with_joins in from.iter().skip(1) {
            let right_plan = self.plan_table_factor(&table_with_joins.relation)?;
            let mut current_right = right_plan;
            for join in &table_with_joins.joins {
                let next_right = self.plan_table_factor(&join.relation)?;
                current_right = self.plan_join(current_right, next_right, &join.join_operator)?;
            }
            plan = LogicalPlan::CrossJoin {
                left: Box::new(plan),
                right: Box::new(current_right),
            };
        }
        
        Ok(plan)
    }

    fn plan_table_factor(&mut self, factor: &TableFactor) -> Result<LogicalPlan, String> {
        match factor {
            TableFactor::Table { name, .. } => {
                let table_name = name.to_string();
                if let Some(cte_plan) = self.ctes.get(&table_name) {
                    Ok(cte_plan.clone())
                } else {
                    Ok(LogicalPlan::Scan {
                        source_id: table_name,
                        projection: None,
                        options: None,
                        schema: None,
                    })
                }
            }
            TableFactor::Derived { subquery, .. } => {
                self.plan_query(subquery)
            }
            _ => Err(format!("Unsupported table factor: {:?}", factor)),
        }
    }

    fn plan_join(&mut self, left: LogicalPlan, right: LogicalPlan, op: &sqlparser::ast::JoinOperator) -> Result<LogicalPlan, String> {
        use sqlparser::ast::{JoinOperator, JoinConstraint};
        use crate::runtime::execution::df_engine::JoinType;

        match op {
            JoinOperator::Inner(constraint) | 
            JoinOperator::LeftOuter(constraint) | 
            JoinOperator::RightOuter(constraint) | 
            JoinOperator::FullOuter(constraint) => {
                let join_type = match op {
                    JoinOperator::Inner(_) => JoinType::Inner,
                    JoinOperator::LeftOuter(_) => JoinType::Left,
                    JoinOperator::RightOuter(_) => JoinType::Right,
                    JoinOperator::FullOuter(_) => JoinType::Full,
                    _ => unreachable!(),
                };

                let (on_left, on_right) = match constraint {
                    JoinConstraint::On(expr) => {
                        // For Production Zero: We only support simple BinaryExpr equality for ON
                        // Example: ON a.id = b.id
                        if let sqlparser::ast::Expr::BinaryOp { left, op: sqlparser::ast::BinaryOperator::Eq, right } = expr {
                            (left.to_string().split('.').last().unwrap().to_string(), 
                             right.to_string().split('.').last().unwrap().to_string())
                        } else {
                            return Err("Only simple equality constraints supported in ON clause".to_string());
                        }
                    }
                    _ => return Err("Only ON constraint supported for Joins".to_string()),
                };

                Ok(LogicalPlan::Join {
                    left: Box::new(left),
                    right: Box::new(right),
                    on_left,
                    on_right,
                    join_type,
                })
            }
            JoinOperator::CrossJoin => {
                Ok(LogicalPlan::CrossJoin {
                    left: Box::new(left),
                    right: Box::new(right),
                })
            }
            _ => Err(format!("Unsupported join operator: {:?}", op)),
        }
    }

    fn plan_projection(&mut self, input: LogicalPlan, projection: &[SelectItem]) -> Result<LogicalPlan, String> {
        let mut exprs = Vec::new();
        let mut names = Vec::new();

        for item in projection {
            match item {
                SelectItem::UnnamedExpr(expr) => {
                    exprs.push(self.map_expr(expr)?);
                    names.push(expr.to_string());
                }
                SelectItem::ExprWithAlias { expr, alias } => {
                    exprs.push(self.map_expr(expr)?);
                    names.push(alias.to_string());
                }
                SelectItem::Wildcard(_) => {
                    // For now, Wildcard in projection is handled by returning all from input
                    // In a full system, we'd expand this based on input schema
                    return Ok(input);
                }
                _ => return Err(format!("Unsupported projection item: {:?}", item)),
            }
        }

        Ok(LogicalPlan::Projection {
            input: Box::new(input),
            exprs,
            names,
        })
    }

    fn plan_aggregate(&mut self, input: LogicalPlan, group_by: &sqlparser::ast::GroupByExpr, projection: &[SelectItem]) -> Result<LogicalPlan, String> {
        let mut keys = Vec::new();
        let mut key_names = Vec::new();
        
        match group_by {
            sqlparser::ast::GroupByExpr::Expressions(exprs, _) => {
                for expr in exprs {
                    keys.push(self.map_expr(expr)?);
                    key_names.push(expr.to_string());
                }
            }
            _ => return Err("Only explicit GroupBy expressions supported".to_string()),
        }

        let mut aggs = Vec::new();
        let mut ops = Vec::new();
        let mut agg_names = Vec::new();

        for item in projection {
            match item {
                SelectItem::UnnamedExpr(expr) => {
                    if let Some((agg_expr, op)) = self.extract_aggregate(expr)? {
                        aggs.push(agg_expr);
                        ops.push(op);
                        agg_names.push(expr.to_string());
                    }
                }
                SelectItem::ExprWithAlias { expr, alias } => {
                    if let Some((agg_expr, op)) = self.extract_aggregate(expr)? {
                        aggs.push(agg_expr);
                        ops.push(op);
                        agg_names.push(alias.to_string());
                    }
                }
                _ => {}
            }
        }

        Ok(LogicalPlan::Aggregate {
            input: Box::new(input),
            keys,
            aggs,
            ops,
            key_names,
            agg_names,
        })
    }

    fn map_expr(&mut self, expr: &SqlExpr) -> Result<NyxExpr, String> {
        match expr {
            SqlExpr::Identifier(ident) => Ok(NyxExpr::Identifier(ident.to_string())),
            SqlExpr::BinaryOp { left, op, right } => {
                let l = Box::new(self.map_expr(left)?);
                let r = Box::new(self.map_expr(right)?);
                let nyx_op = match op {
                    BinaryOperator::Eq => "==".to_string(),
                    BinaryOperator::NotEq => "!=".to_string(),
                    BinaryOperator::Lt => "<".to_string(),
                    BinaryOperator::LtEq => "<=".to_string(),
                    BinaryOperator::Gt => ">".to_string(),
                    BinaryOperator::GtEq => ">=".to_string(),
                    BinaryOperator::Plus => "+".to_string(),
                    BinaryOperator::Minus => "-".to_string(),
                    BinaryOperator::Multiply => "*".to_string(),
                    BinaryOperator::Divide => "/".to_string(),
                    _ => return Err(format!("Unsupported binary operator: {:?}", op)),
                };
                Ok(NyxExpr::Binary { left: l, op: nyx_op, right: r })
            }
            SqlExpr::Value(val) => {
                use sqlparser::ast::Value;
                match val {
                    Value::Number(n, _) => {
                        if n.contains('.') {
                            Ok(NyxExpr::FloatLiteral(n.parse().unwrap_or(0.0)))
                        } else {
                            Ok(NyxExpr::IntLiteral(n.parse().unwrap_or(0)))
                        }
                    }
                    Value::SingleQuotedString(s) => Ok(NyxExpr::StringLiteral(s.clone())),
                    Value::Boolean(b) => Ok(NyxExpr::BoolLiteral(*b)),
                    Value::Null => Ok(NyxExpr::Identifier("null".to_string())),
                    _ => Err(format!("Unsupported value type: {:?}", val)),
                }
            }
            SqlExpr::Subquery(subquery) => {
                let _plan = self.plan_query(subquery)?;
                // For now, return a placeholder identifier to avoid crash
                // Full scalar subquery integration requires VM support
                Ok(NyxExpr::Identifier(format!("subquery_{}", uuid::Uuid::new_v4().simple())))
            }
            _ => Err(format!("Unsupported SQL expression: {:?}", expr)),
        }
    }

    fn extract_aggregate(&mut self, expr: &SqlExpr) -> Result<Option<(NyxExpr, AggregateOp)>, String> {
        if let SqlExpr::Function(func) = expr {
            let name = func.name.to_string().to_uppercase();
            let op = match name.as_str() {
                "SUM" => Some(AggregateOp::Sum),
                "COUNT" => Some(AggregateOp::Count),
                "AVG" | "MEAN" => Some(AggregateOp::Mean),
                "MIN" => Some(AggregateOp::Min),
                "MAX" => Some(AggregateOp::Max),
                _ => None,
            };

            if let Some(op) = op {
                use sqlparser::ast::FunctionArguments;
                match &func.args {
                    FunctionArguments::None => {
                        return Ok(Some((NyxExpr::IntLiteral(1), AggregateOp::Count)));
                    }
                    FunctionArguments::List(list) => {
                        if list.args.is_empty() {
                            return Ok(Some((NyxExpr::IntLiteral(1), AggregateOp::Count)));
                        }
                        use sqlparser::ast::FunctionArg;
                        let arg_expr = match &list.args[0] {
                            FunctionArg::Unnamed(arg_expr) => match arg_expr {
                                sqlparser::ast::FunctionArgExpr::Expr(e) => self.map_expr(e)?,
                                sqlparser::ast::FunctionArgExpr::Wildcard => NyxExpr::IntLiteral(1),
                                _ => return Err("Unsupported function argument expression".to_string()),
                            },
                            _ => return Err("Only unnamed arguments supported".to_string()),
                        };
                        return Ok(Some((arg_expr, op)));
                    }
                    _ => return Err("Unsupported function arguments".to_string()),
                }
            }
        }
        Ok(None)
    }

    fn has_aggregates(&mut self, projection: &[SelectItem]) -> bool {
        for item in projection {
            match item {
                SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. } => {
                    if self.extract_aggregate(expr).unwrap_or(None).is_some() {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }
}

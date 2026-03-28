use std::collections::HashMap;

use crate::core::parser::syntax_rules::SyntaxRules;
use crate::core::registry::language_registry::LanguageRegistry;

#[derive(Debug, Clone)]
pub struct GrammarNode {
    pub symbol: String,
    pub edges: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct GrammarGraph {
    pub nodes: HashMap<String, GrammarNode>,
}

#[derive(Debug, Clone)]
pub struct GrammarEngine {
    pub graph: GrammarGraph,
    pub syntax_rules: SyntaxRules,
}

impl GrammarEngine {
    pub fn from_registry(registry: &LanguageRegistry) -> Self {
        let mut graph = GrammarGraph::default();

        graph.nodes.insert(
            "Program".to_string(),
            GrammarNode {
                symbol: "Program".to_string(),
                edges: vec!["FunctionDecl".to_string()],
            },
        );
        graph.nodes.insert(
            "FunctionDecl".to_string(),
            GrammarNode {
                symbol: "FunctionDecl".to_string(),
                edges: vec!["Stmt".to_string()],
            },
        );
        graph.nodes.insert(
            "Stmt".to_string(),
            GrammarNode {
                symbol: "Stmt".to_string(),
                edges: vec![
                    "LetStmt".to_string(),
                    "ExprStmt".to_string(),
                    "ReturnStmt".to_string(),
                    "DeferStmt".to_string(),
                ],
            },
        );
        graph.nodes.insert(
            "DeferStmt".to_string(),
            GrammarNode {
                symbol: "DeferStmt".to_string(),
                edges: vec!["Stmt".to_string()],
            },
        );
        graph.nodes.insert(
            "LetStmt".to_string(),
            GrammarNode {
                symbol: "LetStmt".to_string(),
                edges: vec!["Expr".to_string()],
            },
        );
        graph.nodes.insert(
            "ExprStmt".to_string(),
            GrammarNode {
                symbol: "ExprStmt".to_string(),
                edges: vec!["Expr".to_string()],
            },
        );
        graph.nodes.insert(
            "ReturnStmt".to_string(),
            GrammarNode {
                symbol: "ReturnStmt".to_string(),
                edges: vec!["Expr".to_string()],
            },
        );
        graph.nodes.insert(
            "Expr".to_string(),
            GrammarNode {
                symbol: "Expr".to_string(),
                edges: vec!["Primary".to_string(), "Expr".to_string()],
            },
        );
        graph.nodes.insert(
            "Primary".to_string(),
            GrammarNode {
                symbol: "Primary".to_string(),
                edges: vec![
                    "kw:let".to_string(),
                    "kw:return".to_string(),
                    "CallExpr".to_string(),
                ],
            },
        );
        graph.nodes.insert(
            "CallExpr".to_string(),
            GrammarNode {
                symbol: "CallExpr".to_string(),
                edges: vec!["Expr".to_string()],
            },
        );

        for keyword in &registry.keywords {
            graph.nodes.insert(
                format!("kw:{keyword}"),
                GrammarNode {
                    symbol: format!("kw:{keyword}"),
                    edges: vec![],
                },
            );
        }

        for op in &registry.operators {
            graph.nodes.insert(
                format!("op:{op}"),
                GrammarNode {
                    symbol: format!("op:{op}"),
                    edges: vec![],
                },
            );
        }

        Self {
            graph,
            syntax_rules: SyntaxRules::new(registry.syntax_patterns.clone()),
        }
    }

    pub fn validate_determinism(&self, registry: &LanguageRegistry) -> Result<(), String> {
        ensure_no_duplicates(&registry.keywords, "keywords")?;
        ensure_no_duplicates(&registry.operators, "operators")?;
        ensure_operator_set_supported(&registry.operators)?;
        ensure_required_patterns(&self.syntax_rules)?;
        self.validate_graph()?;
        Ok(())
    }

    fn validate_graph(&self) -> Result<(), String> {
        for (name, node) in &self.graph.nodes {
            if node.symbol != *name {
                return Err(format!(
                    "grammar node symbol mismatch: key '{name}' != symbol '{}'",
                    node.symbol
                ));
            }
            for edge in &node.edges {
                if !self.graph.nodes.contains_key(edge) {
                    return Err(format!(
                        "grammar edge from '{name}' points to unknown node '{edge}'"
                    ));
                }
            }
        }

        let root = "Program";
        if !self.graph.nodes.contains_key(root) {
            return Err("grammar missing required root node 'Program'".to_string());
        }

        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![root.to_string()];
        while let Some(current) = stack.pop() {
            if !visited.insert(current.clone()) {
                continue;
            }
            if let Some(node) = self.graph.nodes.get(&current) {
                for edge in &node.edges {
                    stack.push(edge.clone());
                }
            }
        }

        for required in [
            "FunctionDecl",
            "Stmt",
            "LetStmt",
            "ExprStmt",
            "ReturnStmt",
            "Expr",
        ] {
            if !visited.contains(required) {
                return Err(format!(
                    "required grammar state '{required}' is unreachable from Program"
                ));
            }
        }

        Ok(())
    }
}

fn ensure_no_duplicates(values: &[String], label: &str) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(format!("duplicate entry '{value}' in {label}"));
        }
    }
    Ok(())
}

fn ensure_operator_set_supported(operators: &[String]) -> Result<(), String> {
    // Support all operators from the language registry
    const SUPPORTED: &[&str] = &[
        "+", "-", "*", "/", "%", "=", "==", "!", "!=", "<", "<=", ">", ">=",
        "&", "&&", "|", "||", "^", "~", "<<", ">>", "+=", "-=", "*=", "/=",
        "%=", "&=", "|=", "^=", "<<=", ">>=", "(", ")", "{", "}", "[", "]",
        ",", ":", "::", ".", "..", "..=", "...", ";", "?", "??", "@", "->", "=>", "<-"
    ];
    for op in operators {
        if !SUPPORTED.contains(&op.as_str()) {
            return Err(format!(
                "operator '{op}' is not supported by deterministic parser/IR core"
            ));
        }
    }
    Ok(())
}

fn ensure_required_patterns(rules: &SyntaxRules) -> Result<(), String> {
    let p = &rules.patterns;
    if !p.function.contains("fn") || !(p.function.contains("<stmt>*") || p.function.contains("<body>")) {
        return Err("function syntax pattern must include 'fn' and '<stmt>*' or '<body>'".to_string());
    }
    if !p.let_stmt.contains("let") || !p.let_stmt.contains("<expr>") {
        return Err("let syntax pattern must include 'let' and '<expr>'".to_string());
    }
    if !p.return_stmt.contains("return") {
        return Err("return syntax pattern must include 'return'".to_string());
    }
    if !p.defer_stmt.contains("defer") {
        return Err("defer syntax pattern must include 'defer'".to_string());
    }
    if !p.call_expr.contains("<ident>") {
        return Err("call expression syntax pattern must include '<ident>'".to_string());
    }
    Ok(())
}

use crate::core::ast::ast_nodes::*;
use crate::core::lexer::token::Span;

pub struct ProtocolLowerer;

impl ProtocolLowerer {
    pub fn lower(program: &mut Program) {
        let mut new_items = Vec::new();
        let old_items = std::mem::take(&mut program.items);
        for item in old_items {
            let vis = item.vis.clone(); // Capture visibility
            if let ItemKind::Protocol(proto) = &item.kind {
                new_items.extend(Self::lower_protocol(proto.clone(), vis.clone()));
            }
            new_items.push(item);
        }
        program.items = new_items;
    }

    fn lower_protocol(proto: ProtocolDecl, vis: Visibility) -> Vec<Item> {
        let mut items = Vec::new();
        let span = proto.span.clone();

        // 1. Generate role structs and impls
        for role in &proto.roles {
            let struct_name = format!("{}_{}", proto.name, role);
            
            // Struct definition
            let fields = vec![
                StructField {
                    vis: Visibility::Public,
                    name: "state".to_string(),
                    field_type: Type::simple("int"),
                    default: Some(Expr::IntLiteral(0)),
                },
                StructField {
                    vis: Visibility::Public,
                    name: "transcript".to_string(),
                    field_type: Type::simple("bytes"),
                    default: None,
                },
                StructField {
                    vis: Visibility::Public,
                    name: "session_key".to_string(),
                    field_type: Type::simple("bytes"),
                    default: None,
                }
            ];

            let s_decl = StructDecl {
                name: struct_name.clone(),
                fields,
                generics: Vec::new(),
                where_clauses: Vec::new(),
                span: span.clone(),
            };

            items.push(Item {
                attributes: Vec::new(),
                vis: vis.clone(),
                kind: ItemKind::Struct(s_decl),
                span: span.clone(),
            });

            // Impl block
            let mut impl_items = Vec::new();
            
            // Constructor
            impl_items.push(ImplItem::Method(FunctionDecl {
                name: "new".to_string(),
                is_async: false,
                is_extern: false,
                extern_abi: None,
                generics: Vec::new(),
                params: Vec::new(),
                return_type: Some(Type::simple(&struct_name)),
                where_clauses: Vec::new(),
                body: vec![
                    Stmt::Return {
                        expr: Some(Expr::StructLiteral {
                            name: struct_name.clone(),
                            fields: vec![
                                FieldInit { name: "state".to_string(), value: Expr::IntLiteral(0) },
                                FieldInit { 
                                    name: "transcript".to_string(), 
                                    value: Expr::Call {
                                        callee: Box::new(Expr::Path(vec!["bytes".to_string(), "new".to_string()])),
                                        args: vec![],
                                    }
                                },
                                FieldInit { 
                                    name: "session_key".to_string(), 
                                    value: Expr::Call {
                                        callee: Box::new(Expr::Path(vec!["crypto".to_string(), "random_bytes".to_string()])),
                                        args: vec![Expr::IntLiteral(32)],
                                    }
                                },
                            ],
                        }),
                        span: span.clone(),
                    }
                ],
                span: span.clone(),
            }));

            // Protocol-specific methods
            if let Some(handshake) = &proto.handshake {
                for (i, step) in handshake.steps.iter().enumerate() {
                    match step {
                        HandshakeStep::Message { from, to, name, fields } => {
                            if from == role {
                                impl_items.push(ImplItem::Method(Self::generate_send_method(name, to, fields, i as i32, span.clone())));
                            } else if to == role {
                                impl_items.push(ImplItem::Method(Self::generate_recv_method(name, from, fields, i as i32, span.clone())));
                            }
                        }
                        HandshakeStep::Derive { assignments } => {
                             impl_items.push(ImplItem::Method(Self::generate_derive_method(assignments, i as i32, span.clone())));
                        }
                        HandshakeStep::Finish { actions } => {
                             impl_items.push(ImplItem::Method(Self::generate_finish_method(actions, i as i32, span.clone())));
                        }
                    }
                }
            }

            items.push(Item {
                attributes: Vec::new(),
                vis: vis.clone(),
                kind: ItemKind::Impl(ImplBlock {
                    generics: Vec::new(),
                    trait_name: None,
                    self_type: Type::simple(&struct_name),
                    items: impl_items,
                    span: span.clone(),
                }),
                span: span.clone(),
            });
        }

        items
    }

    fn generate_send_method(name: &str, to: &str, _fields: &Vec<HandshakeField>, step_idx: i32, span: Span) -> FunctionDecl {
        FunctionDecl {
            name: format!("send_{}", name),
            is_async: false,
            is_extern: false,
            extern_abi: None,
            generics: Vec::new(),
            params: vec![Param { 
                name: "self".to_string(), 
                mutable: false,
                param_type: Type::simple("Self"),
                default_value: None 
            }],
            return_type: Some(Type::simple("bytes")),
            where_clauses: Vec::new(),
            body: vec![
                Stmt::Print { expr: Expr::StringLiteral(format!("[NYX-PROTO] Sending {} to {}", name, to)) },
                Stmt::Assign {
                    target: Expr::FieldAccess { object: Box::new(Expr::Identifier("self".to_string())), field: "state".to_string() },
                    value: Expr::IntLiteral(step_idx as i64 + 1),
                    span: span.clone(),
                },
                Stmt::Return {
                    expr: Some(Expr::Call {
                        callee: Box::new(Expr::Path(vec!["crypto".to_string(), "seal_ephemeral".to_string()])),
                        args: vec![
                            Expr::Call {
                                callee: Box::new(Expr::Path(vec!["bytes".to_string(), "from_str".to_string()])),
                                args: vec![Expr::StringLiteral(format!("{} handshake", name))],
                            },
                            Expr::FieldAccess {
                                object: Box::new(Expr::Identifier("self".to_string())),
                                field: "session_key".to_string(),
                            }
                        ],
                    }),
                    span: span.clone(),
                }
            ],
            span: span.clone(),
        }
    }

    fn generate_recv_method(name: &str, from: &str, _fields: &Vec<HandshakeField>, step_idx: i32, span: Span) -> FunctionDecl {
        FunctionDecl {
            name: format!("recv_{}", name),
            is_async: false,
            is_extern: false,
            extern_abi: None,
            generics: Vec::new(),
            params: vec![
                Param { 
                    name: "self".to_string(), 
                    mutable: false,
                    param_type: Type::simple("Self"),
                    default_value: None 
                },
                Param { 
                    name: "packet".to_string(), 
                    mutable: false,
                    param_type: Type::simple("bytes"),
                    default_value: None 
                }
            ],
            return_type: Some(Type::simple("bool")),
            where_clauses: Vec::new(),
            body: vec![
                Stmt::Print { expr: Expr::StringLiteral(format!("[NYX-PROTO] Receiving {} from {}", name, from)) },
                Stmt::Assign {
                    target: Expr::FieldAccess { object: Box::new(Expr::Identifier("self".to_string())), field: "state".to_string() },
                    value: Expr::IntLiteral(step_idx as i64 + 1),
                    span: span.clone(),
                },
                Stmt::Return {
                    expr: Some(Expr::BoolLiteral(true)),
                    span: span.clone(),
                }
            ],
            span: span.clone(),
        }
    }

    fn generate_derive_method(assignments: &Vec<HandshakeAssignment>, step_idx: i32, span: Span) -> FunctionDecl {
        FunctionDecl {
            name: format!("step_{}_derive", step_idx),
            is_async: false,
            is_extern: false,
            extern_abi: None,
            generics: Vec::new(),
            params: vec![Param { 
                name: "self".to_string(), 
                mutable: false,
                param_type: Type::simple("Self"),
                default_value: None 
            }],
            return_type: Some(Type::simple("bool")),
            where_clauses: Vec::new(),
            body: {
                let mut stmts: Vec<Stmt> = assignments.iter().map(|a| Stmt::Print { expr: Expr::StringLiteral(format!("[NYX-PROTO] Deriving {}", a.name)) }).collect();
                stmts.push(Stmt::Assign {
                    target: Expr::FieldAccess { object: Box::new(Expr::Identifier("self".to_string())), field: "state".to_string() },
                    value: Expr::IntLiteral(step_idx as i64 + 1),
                    span: span.clone(),
                });
                stmts.push(Stmt::Return {
                    expr: Some(Expr::BoolLiteral(true)),
                    span: span.clone(),
                });
                stmts
            },
            span: span.clone(),
        }
    }

    fn generate_finish_method(actions: &Vec<String>, _step_idx: i32, span: Span) -> FunctionDecl {
        FunctionDecl {
            name: "complete_handshake".to_string(),
            is_async: false,
            is_extern: false,
            extern_abi: None,
            generics: Vec::new(),
            params: vec![Param { 
                name: "self".to_string(), 
                mutable: false,
                param_type: Type::simple("Self"),
                default_value: None 
            }],
            return_type: Some(Type::simple("bool")),
            where_clauses: Vec::new(),
            body: {
                let mut stmts: Vec<Stmt> = actions.iter().map(|a| Stmt::Print { expr: Expr::StringLiteral(format!("[NYX-PROTO] Action: {}", a)) }).collect();
                stmts.push(Stmt::Assign {
                    target: Expr::FieldAccess { object: Box::new(Expr::Identifier("self".to_string())), field: "state".to_string() },
                    value: Expr::IntLiteral(4), // Terminal state
                    span: span.clone(),
                });
                stmts.push(Stmt::Return {
                    expr: Some(Expr::BoolLiteral(true)),
                    span: span.clone(),
                });
                stmts
            },
            span: span.clone(),
        }
    }
}

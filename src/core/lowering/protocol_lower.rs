use crate::core::ast::ast_nodes::*;
use crate::core::diagnostics::Span;

pub struct ProtocolLowerer;

impl ProtocolLowerer {
    pub fn lower(program: &mut Program) {
        let mut new_items = Vec::new();
        let old_items = std::mem::take(&mut program.items);
        for item in old_items {
            let vis = item.vis.clone();
            if let ItemKind::Protocol(proto) = &item.kind {
                new_items.extend(Self::lower_protocol(proto.clone(), vis.clone()));
            }
            new_items.push(item);
        }
        program.items = new_items;
    }

    fn lower_protocol(proto: ProtocolDecl, vis: Visibility) -> Vec<Item> {
        let mut items = Vec::new();
        let span = proto.span;

        for role in &proto.roles {
            let struct_name = format!("{}_{}", proto.name, role);

            let fields = vec![
                StructField {
                    vis: Visibility::Public,
                    name: "state".to_string(),
                    field_type: Type::simple("int"),
                    default: Some(Expr::int(0)),
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
                },
            ];

            let s_decl = StructDecl {
                name: struct_name.clone(),
                fields,
                generics: Vec::new(),
                where_clauses: Vec::new(),
                span,
            };

            items.push(Item {
                attributes: Vec::new(),
                vis: vis.clone(),
                kind: ItemKind::Struct(s_decl),
                span,
            });

            let mut impl_items = Vec::new();

            impl_items.push(ImplItem::Method(FunctionDecl {
                name: "new".to_string(),
                is_async: false,
                is_extern: false,
                extern_abi: None,
                generics: Vec::new(),
                params: Vec::new(),
                return_type: Some(Type::simple(&struct_name)),
                where_clauses: Vec::new(),
                body: vec![Stmt::Return {
                    expr: Some(Expr::StructLiteral {
                        name: struct_name.clone(),
                        fields: vec![
                            FieldInit { name: "state".to_string(), value: Expr::int(0) },
                            FieldInit {
                                name: "transcript".to_string(),
                                value: Expr::Call {
                                    callee: Box::new(Expr::Path {
                                        segments: vec!["bytes".to_string(), "new".to_string()],
                                        span,
                                    }),
                                    args: vec![],
                                    span,
                                },
                            },
                            FieldInit {
                                name: "session_key".to_string(),
                                value: Expr::Call {
                                    callee: Box::new(Expr::Path {
                                        segments: vec!["crypto".to_string(), "random_bytes".to_string()],
                                        span,
                                    }),
                                    args: vec![Expr::int(32)],
                                    span,
                                },
                            },
                        ],
                        span,
                    }),
                    span,
                }],
                span,
            }));

            if let Some(handshake) = &proto.handshake {
                for (i, step) in handshake.steps.iter().enumerate() {
                    match step {
                        HandshakeStep::Message { from, to, name, fields } => {
                            if from == role {
                                impl_items.push(ImplItem::Method(Self::generate_send_method(
                                    name, to, fields, i as i32, span,
                                )));
                            } else if to == role {
                                impl_items.push(ImplItem::Method(Self::generate_recv_method(
                                    name, from, fields, i as i32, span,
                                )));
                            }
                        }
                        HandshakeStep::Derive { assignments } => {
                            impl_items.push(ImplItem::Method(Self::generate_derive_method(
                                assignments, i as i32, span,
                            )));
                        }
                        HandshakeStep::Finish { actions } => {
                            impl_items.push(ImplItem::Method(Self::generate_finish_method(
                                actions, i as i32, span,
                            )));
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
                    span,
                }),
                span,
            });
        }

        items
    }

    fn generate_send_method(
        name: &str,
        to: &str,
        _fields: &Vec<HandshakeField>,
        step_idx: i32,
        span: Span,
    ) -> FunctionDecl {
        FunctionDecl {
            name: format!("send_{}", name),
            is_async: false,
            is_extern: false,
            extern_abi: None,
            generics: Vec::new(),
            params: vec![Param {
                name: "self".to_string(),
                mutable: false,
                is_variadic: false,
                param_type: Type::simple("Self"),
                default_value: None,
            }],
            return_type: Some(Type::simple("bytes")),
            where_clauses: Vec::new(),
            body: vec![
                Stmt::Print {
                    expr: Expr::string(format!("[NYX-PROTO] Sending {} to {}", name, to)),
                },
                Stmt::Assign {
                    target: Expr::FieldAccess {
                        object: Box::new(Expr::ident("self")),
                        field: "state".to_string(),
                        span,
                    },
                    value: Expr::int(step_idx as i64 + 1),
                    span,
                },
                Stmt::Return {
                    expr: Some(Expr::Call {
                        callee: Box::new(Expr::Path {
                            segments: vec!["crypto".to_string(), "seal_ephemeral".to_string()],
                            span,
                        }),
                        args: vec![
                            Expr::Call {
                                callee: Box::new(Expr::Path {
                                    segments: vec!["bytes".to_string(), "from_str".to_string()],
                                    span,
                                }),
                                args: vec![Expr::string(format!("{} handshake", name))],
                                span,
                            },
                            Expr::FieldAccess {
                                object: Box::new(Expr::ident("self")),
                                field: "session_key".to_string(),
                                span,
                            },
                        ],
                        span,
                    }),
                    span,
                },
            ],
            span,
        }
    }

    fn generate_recv_method(
        name: &str,
        from: &str,
        _fields: &Vec<HandshakeField>,
        step_idx: i32,
        span: Span,
    ) -> FunctionDecl {
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
                    is_variadic: false,
                    param_type: Type::simple("Self"),
                    default_value: None,
                },
                Param {
                    name: "packet".to_string(),
                    mutable: false,
                    is_variadic: false,
                    param_type: Type::simple("bytes"),
                    default_value: None,
                },
            ],
            return_type: Some(Type::simple("bool")),
            where_clauses: Vec::new(),
            body: vec![
                Stmt::Print {
                    expr: Expr::string(format!("[NYX-PROTO] Receiving {} from {}", name, from)),
                },
                Stmt::Assign {
                    target: Expr::FieldAccess {
                        object: Box::new(Expr::ident("self")),
                        field: "state".to_string(),
                        span,
                    },
                    value: Expr::int(step_idx as i64 + 1),
                    span,
                },
                Stmt::Return {
                    expr: Some(Expr::bool(true)),
                    span,
                },
            ],
            span,
        }
    }

    fn generate_derive_method(
        assignments: &Vec<HandshakeAssignment>,
        step_idx: i32,
        span: Span,
    ) -> FunctionDecl {
        FunctionDecl {
            name: format!("step_{}_derive", step_idx),
            is_async: false,
            is_extern: false,
            extern_abi: None,
            generics: Vec::new(),
            params: vec![Param {
                name: "self".to_string(),
                mutable: false,
                is_variadic: false,
                param_type: Type::simple("Self"),
                default_value: None,
            }],
            return_type: Some(Type::simple("bool")),
            where_clauses: Vec::new(),
            body: {
                let mut stmts: Vec<Stmt> = assignments
                    .iter()
                    .map(|a| Stmt::Print {
                        expr: Expr::string(format!("[NYX-PROTO] Deriving {}", a.name)),
                    })
                    .collect();
                stmts.push(Stmt::Assign {
                    target: Expr::FieldAccess {
                        object: Box::new(Expr::ident("self")),
                        field: "state".to_string(),
                        span,
                    },
                    value: Expr::int(step_idx as i64 + 1),
                    span,
                });
                stmts.push(Stmt::Return {
                    expr: Some(Expr::bool(true)),
                    span,
                });
                stmts
            },
            span,
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
                is_variadic: false,
                param_type: Type::simple("Self"),
                default_value: None,
            }],
            return_type: Some(Type::simple("bool")),
            where_clauses: Vec::new(),
            body: {
                let mut stmts: Vec<Stmt> = actions
                    .iter()
                    .map(|a| Stmt::Print {
                        expr: Expr::string(format!("[NYX-PROTO] Action: {}", a)),
                    })
                    .collect();
                stmts.push(Stmt::Assign {
                    target: Expr::FieldAccess {
                        object: Box::new(Expr::ident("self")),
                        field: "state".to_string(),
                        span,
                    },
                    value: Expr::int(4),
                    span,
                });
                stmts.push(Stmt::Return {
                    expr: Some(Expr::bool(true)),
                    span,
                });
                stmts
            },
            span,
        }
    }
}

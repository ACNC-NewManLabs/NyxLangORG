#[cfg(test)]
mod tests {
    use crate::surn::*;
    use crate::surn::lexer::TokenKind;

    #[test]
    fn test_lexer_basic() {
        let input = "name = \"nyx\"\nversion = 1.0\n[package]";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Identifier(ref s) if s == "name")));
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::String(ref s) if s == "nyx")));
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Float(f) if f == 1.0)));
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::LeftBracket)));
    }

    #[test]
    fn test_parser_config_mode() {
        let input = "[package]\nname = \"nyx\"\nversion = \"1.0\"";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens, input.to_string());
        let doc = parser.parse().unwrap();

        assert_eq!(doc.statements.len(), 1);
        if let Statement::Table { header, assignments, .. } = &doc.statements[0] {
        assert_eq!(header, &vec!["package".to_string()]);
            assert_eq!(assignments.len(), 2);
        } else {
            panic!("Expected Table statement");
        }
    }

    #[test]
    fn test_parser_object_mode() {
        let input = "api = { endpoint: \"/users\", port: 8080 }";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens, input.to_string());
        let doc = parser.parse().unwrap();

        if let Statement::Assignment { key, value } = &doc.statements[0] {
            assert_eq!(key, "api");
            if let Value::Object(map) = value {
                assert_eq!(map.len(), 2);
                assert!(map.contains_key("endpoint"));
                assert!(map.contains_key("port"));
            } else {
                panic!("Expected Object value");
            }
        }
    }

    #[test]
    fn test_parser_block_mode() {
        let input = "deploy:\n    region = \"us-east\"\n    replicas = 3\n";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens, input.to_string());
        let doc = parser.parse().unwrap();

        if let Statement::Block { key, children } = &doc.statements[0] {
            assert_eq!(key, "deploy");
            assert_eq!(children.len(), 2);
        }
    }

    #[test]
    fn test_serializer() {
        let input = "[package ]\nname = \"nyx\"\n";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens, input.to_string());
        let doc = parser.parse().unwrap();
        
        let output = Serializer::serialize(&doc);
        assert!(output.contains("[package ]"));
        assert!(output.contains("name = \"nyx\""));
    }

    #[test]
    fn test_complex_dependencies() {
        use crate::package_manager::NyxCargo;
        let input = r#"
[dependencies]
log = "0.4"
cranelift-codegen = { version: "0.110", optional: true, features: ["x86", "arm64"] }
"#;
        let cargo = NyxCargo::parse(input).unwrap();
        assert_eq!(cargo.dependencies.len(), 2);
        
        let log = cargo.dependencies.get("log").unwrap();
        if let crate::package_manager::Dependency::Simple(v) = log {
            assert_eq!(v, "0.4");
        } else { panic!("log should be simple"); }

        let cl = cargo.dependencies.get("cranelift-codegen").unwrap();
        if let crate::package_manager::Dependency::Detailed(det) = cl {
            assert_eq!(det.version.as_ref().unwrap(), "0.110");
            assert_eq!(det.optional.unwrap(), true);
            assert_eq!(det.features.as_ref().unwrap().len(), 2);
        } else { panic!("cranelift-codegen should be detailed"); }
    }

    #[test]
    fn test_git_dependencies() {
        use crate::package_manager::NyxCargo;
        let input = r#"
[dependencies]
nyx-utils = { git: "https://github.com/nyx-lang/utils", branch: "main" }
nyx-core = { git: "https://github.com/nyx-lang/core", tag: "v0.1.0" }
"#;
        let cargo = NyxCargo::parse(input).unwrap();
        assert_eq!(cargo.dependencies.len(), 2);
        
        let utils = cargo.dependencies.get("nyx-utils").unwrap();
        if let crate::package_manager::Dependency::Detailed(det) = utils {
            assert_eq!(det.git.as_ref().unwrap(), "https://github.com/nyx-lang/utils");
            assert_eq!(det.branch.as_ref().unwrap(), "main");
        } else { panic!("nyx-utils should be detailed"); }

        let core = cargo.dependencies.get("nyx-core").unwrap();
        if let crate::package_manager::Dependency::Detailed(det) = core {
            assert_eq!(det.git.as_ref().unwrap(), "https://github.com/nyx-lang/core");
            assert_eq!(det.tag.as_ref().unwrap(), "v0.1.0");
        } else { panic!("nyx-core should be detailed"); }
    }
}

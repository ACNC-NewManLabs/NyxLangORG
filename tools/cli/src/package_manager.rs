use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::surn::{Lexer, Parser, Statement, Value as SurnValue};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NyxCargo {
    pub package: Package,
    pub dependencies: HashMap<String, Dependency>,
    pub dev_dependencies: HashMap<String, Dependency>,
    pub workspace: Option<Workspace>,
    pub features: HashMap<String, Vec<String>>,
    pub profiles: HashMap<String, Profile>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Profile {
    pub opt_level: u32,
    pub debug: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub edition: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Dependency {
    Simple(String),
    Detailed(DependencyDetail),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DependencyDetail {
    pub version: Option<String>,
    pub git: Option<String>,
    pub branch: Option<String>,
    pub tag: Option<String>,
    pub rev: Option<String>,
    pub path: Option<String>,
    pub optional: Option<bool>,
    pub features: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Workspace {
    pub members: Vec<String>,
}

impl NyxCargo {
    pub fn parse(content: &str) -> Result<Self, String> {
        let mut lexer = Lexer::new(content);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens, content.to_string());
        let doc = parser.parse()?;
        
        // Map SURN AST to NyxCargo
        // This is a simplified mapping for now
        let mut package = Package { name: "".to_string(), version: "".to_string(), edition: "".to_string() };
        let mut dependencies = HashMap::new();
        let mut dev_dependencies = HashMap::new();
        let mut features = HashMap::new();
        let mut profiles = HashMap::new();
        let mut workspace = None;

        for stmt in doc.statements {
            Self::process_statement(stmt, &mut package, &mut dependencies, &mut dev_dependencies, &mut features, &mut profiles, &mut workspace);
        }

        Ok(NyxCargo { package, dependencies, dev_dependencies, workspace, features, profiles })
    }

    fn process_statement(stmt: Statement, package: &mut Package, deps: &mut HashMap<String, Dependency>, dev_deps: &mut HashMap<String, Dependency>, _features: &mut HashMap<String, Vec<String>>, profiles: &mut HashMap<String, Profile>, _workspace: &mut Option<Workspace>) {
        match stmt {
            Statement::Table { header, assignments, .. } => {
                if header == vec!["package"] {
                    for a in assignments {
                        if let Statement::Assignment { key, value } = a {
                            match key.as_str() {
                                "name" => if let SurnValue::String(s) = value { package.name = s; },
                                "version" => if let SurnValue::String(s) = value { package.version = s; },
                                "edition" => if let SurnValue::String(s) = value { package.edition = s; },
                                _ => {}
                            }
                        }
                    }
                } else if header == vec!["dependencies"] {
                    for a in assignments {
                        if let Statement::Assignment { key, value } = a {
                            deps.insert(key, Self::map_value_to_dep(value));
                        }
                    }
                } else if header.len() == 2 && header[0] == "dependencies" {
                    let name = header[1].clone();
                    let mut obj = HashMap::new();
                    for a in assignments {
                        if let Statement::Assignment { key, value } = a {
                            obj.insert(key, value);
                        }
                    }
                    deps.insert(name, Self::map_value_to_dep(SurnValue::Object(obj)));
                } else if header == vec!["dev-dependencies"] {
                    for a in assignments {
                        if let Statement::Assignment { key, value } = a {
                            dev_deps.insert(key, Self::map_value_to_dep(value));
                        }
                    }
                } else if header.len() == 2 && header[0] == "dev-dependencies" {
                    let name = header[1].clone();
                    let mut obj = HashMap::new();
                    for a in assignments {
                        if let Statement::Assignment { key, value } = a {
                            obj.insert(key, value);
                        }
                    }
                    dev_deps.insert(name, Self::map_value_to_dep(SurnValue::Object(obj)));
                } else if header.len() >= 2 && header[0] == "profile" {
                    let mut p = Profile { opt_level: 0, debug: false };
                    for a in assignments {
                        if let Statement::Assignment { key, value } = a {
                            match key.as_str() {
                                "opt-level" => if let SurnValue::Integer(i) = value { p.opt_level = i as u32; },
                                "debug" => if let SurnValue::Boolean(b) = value { p.debug = b; },
                                _ => {}
                            }
                        }
                    }
                    profiles.insert(header[1].clone(), p);
                }
            }
            Statement::Block { key, children } => {
                if key == "dependencies" {
                    for child in children {
                        if let Statement::Assignment { key, value } = child {
                            deps.insert(key, Self::map_value_to_dep(value));
                        }
                    }
                }
            }
            Statement::Assignment { key, value } => {
                // Top level assignments
                if key == "name" { if let SurnValue::String(s) = value { package.name = s; } }
            }
        }
    }

    fn map_value_to_dep(val: SurnValue) -> Dependency {
        match val {
            SurnValue::String(s) => Dependency::Simple(s),
            SurnValue::Object(obj) => {
                let mut det = DependencyDetail { 
                    version: None, git: None, branch: None, tag: None, rev: None, 
                    path: None, optional: None, features: None 
                };
                if let Some(SurnValue::String(v)) = obj.get("version") { det.version = Some(v.clone()); }
                if let Some(SurnValue::String(g)) = obj.get("git") { det.git = Some(g.clone()); }
                if let Some(SurnValue::String(b)) = obj.get("branch") { det.branch = Some(b.clone()); }
                if let Some(SurnValue::String(t)) = obj.get("tag") { det.tag = Some(t.clone()); }
                if let Some(SurnValue::String(r)) = obj.get("rev") { det.rev = Some(r.clone()); }
                if let Some(SurnValue::String(p)) = obj.get("path") { det.path = Some(p.clone()); }
                if let Some(SurnValue::Boolean(b)) = obj.get("optional") { det.optional = Some(*b); }
                if let Some(SurnValue::Array(arr)) = obj.get("features") {
                    let mut feat_vec = Vec::new();
                    for v in arr {
                        if let SurnValue::String(s) = v {
                            feat_vec.push(s.clone());
                        }
                    }
                    det.features = Some(feat_vec);
                }
                Dependency::Detailed(det)
            }
            _ => Dependency::Simple("".to_string()),
        }
    }
}

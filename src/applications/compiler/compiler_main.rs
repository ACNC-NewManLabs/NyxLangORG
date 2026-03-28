use std::fs;
use std::path::{Path, PathBuf};

use crate::core::ast::ast_nodes::{
    BlockItem, Expr, FunctionDecl, IfBranch, ItemKind, MatchArm, MatchBody, ModuleDecl, Program,
    Stmt, UseTree,
};
use crate::core::diagnostics::{codes, Diagnostic, DiagnosticEngine};
use crate::core::lexer::lexer::Lexer;
use crate::core::parser::grammar_engine::GrammarEngine;
use crate::core::parser::neuro_parser::{IncrementalState, NeuroParser, ParserErrors};
use crate::core::registry::engine_registry::EngineRegistry;
use crate::core::registry::language_registry::LanguageRegistry;
use crate::core::semantic::semantic_analyzer::SemanticAnalyzer;
use crate::core::lowering::protocol_lower::ProtocolLowerer;
use crate::runtime::compiler_bridge::incremental::incremental_patch_set;
use crate::runtime::compiler_bridge::package::package_entry;
use crate::runtime::execution::reload::ModulePatch;
use crate::systems::backend::codegen::{compile_llvm_ir_to_binary, write_llvm_ir, CodegenOutput};
use crate::systems::backend::llvm_backend::{LlvmBackend, Target};
use crate::systems::backend::bytecode_backend::BytecodeBackend;
use crate::systems::ir::ir_builder::IrBuilder;

#[derive(Debug)]
pub struct Compiler {
    language_registry: LanguageRegistry,
    engine_registry: EngineRegistry,
    incremental_state: IncrementalState,
}

#[derive(Debug, Clone)]
pub struct CompileOptions {
    pub input: PathBuf,
    pub output_dir: PathBuf,
    pub module_name: String,
    pub target: Target,
    pub emit_binary: bool,
    pub is_shared: bool,
    pub linker_script: Option<PathBuf>,
}

impl Compiler {
    pub fn new() -> Result<Self, String> {
        let language_json = include_str!("../../../registry/language.json");
        let engines_json = include_str!("../../../registry/engines.json");

        let language_registry = LanguageRegistry::load_from_str(language_json)?;
        let mut engine_registry = EngineRegistry::load_from_str(engines_json)?;

        // Resolution logic: make paths absolute if running globally
        let nyx_home = get_nyx_home();
        for engine in &mut engine_registry.engines {
            let path = Path::new(&engine.path);
            if !path.exists() {
                let absolute = nyx_home.join(&engine.path);
                if absolute.exists() {
                    engine.path = absolute.to_string_lossy().to_string();
                }
            }
        }

        Ok(Self {
            language_registry,
            engine_registry,
            incremental_state: IncrementalState::default(),
        })
    }

    pub fn from_registry_files(
        language_path: impl AsRef<Path>,
        engine_path: impl AsRef<Path>,
    ) -> Result<Self, String> {
        let language_registry = LanguageRegistry::load(language_path)?;
        let engine_registry = EngineRegistry::load(engine_path)?;
        Ok(Self {
            language_registry,
            engine_registry,
            incremental_state: IncrementalState::default(),
        })
    }

    pub fn check(&mut self, source: &str) -> Result<(), String> {
        let mut engine = DiagnosticEngine::default();
        let _ = self.frontend_parse(source, &mut engine)?;
        if engine.has_any_errors() {
            return Err(format_diagnostics(&engine));
        }
        Ok(())
    }

    pub fn compile(&mut self, opts: CompileOptions) -> Result<CodegenOutput, String> {
        let package =
            package_entry(&opts.input, opts.target.triple()).map_err(|err| err.message)?;
        let mut module_asts = Vec::new();
        let mut engine = DiagnosticEngine::default();

        for module in &package.package.modules {
            if let Some(ast) = self.frontend_parse(&module.source, &mut engine)? {
                module_asts.push((module.id.clone(), ast));
            }
        }

        if engine.has_any_errors() {
            return Err(format_diagnostics(&engine));
        }

        let function_index = build_function_index(&module_asts, &package.package.entry_module);
        let module_aliases = build_module_aliases(&package.package.modules);
        let mut items = Vec::new();

        for (module_id, mut ast) in module_asts {
            namespace_module_program(
                &mut ast,
                &module_id,
                &package.package.entry_module,
                &function_index,
                &module_aliases,
            );
            items.extend(ast.items);
        }

        let ir_builder = IrBuilder::default();
        let module = ir_builder.build(&Program { items })?;

        if matches!(opts.target, Target::BrowserJs) {
            let backend = crate::systems::backend::js_backend::JsBackend::default();
            let js_code = backend.lower_to_js(&module)?;

            std::fs::create_dir_all(&opts.output_dir).map_err(|e| e.to_string())?;
            let output_js = opts.output_dir.join(format!("{}.js", opts.module_name));
            std::fs::write(&output_js, js_code).map_err(|e| e.to_string())?;

            return Ok(CodegenOutput {
                llvm_ir_path: output_js.display().to_string(),
                binary_path: Some(output_js.display().to_string()),
            });
        }

        if matches!(opts.target, Target::Bytecode) {
            let bc_module = self.compile_to_bytecode(&opts.input)?;
            let bytes = nyx_vm::bytecode::serialize_module(&bc_module)
                .map_err(|e| format!("Serialization error: {}", e))?;

            std::fs::create_dir_all(&opts.output_dir).map_err(|e| e.to_string())?;
            let output_bc = opts.output_dir.join(format!("{}.nyxb", opts.module_name));
            std::fs::write(&output_bc, bytes).map_err(|e| e.to_string())?;

            return Ok(CodegenOutput {
                llvm_ir_path: output_bc.display().to_string(),
                binary_path: Some(output_bc.display().to_string()),
            });
        }

        let backend = LlvmBackend;
        let llvm_ir = backend.lower_to_llvm_ir(&module, opts.target)?;

        let llvm_ir_path = write_llvm_ir(&opts.output_dir, &opts.module_name, &llvm_ir)?;

        let mut binary_path = None;
        if opts.emit_binary {
            let output_binary = opts.output_dir.join(&opts.module_name);
            match compile_llvm_ir_to_binary(
                Path::new(&llvm_ir_path),
                &output_binary,
                opts.target.triple(),
                opts.is_shared,
                opts.linker_script.as_deref(),
            ) {
                Ok(()) => {
                    binary_path = Some(output_binary.display().to_string());
                }
                Err(err) => {
                    // Preserve previous behavior only when clang is missing, so users can still
                    // inspect emitted IR. Any other compilation/link error should surface.
                    if err.contains("clang not found") {
                        binary_path = None;
                    } else {
                        return Err(err);
                    }
                }
            }
        }

        Ok(CodegenOutput {
            llvm_ir_path,
            binary_path,
        })
    }

    pub fn compile_to_bytecode(
        &mut self,
        input: &Path,
    ) -> Result<nyx_vm::bytecode::BytecodeModule, String> {
        let package = package_entry(input, "bytecode").map_err(|err| err.message)?;
        let mut module_asts = Vec::new();
        let mut engine = DiagnosticEngine::default();

        for module in &package.package.modules {
            if let Some(ast) = self.frontend_parse(&module.source, &mut engine)? {
                module_asts.push((module.id.clone(), ast));
            }
        }

        if engine.has_any_errors() {
            return Err(format_diagnostics(&engine));
        }

        let function_index = build_function_index(&module_asts, &package.package.entry_module);
        let module_aliases = build_module_aliases(&package.package.modules);
        let mut items = Vec::new();

        for (module_id, mut ast) in module_asts {
            namespace_module_program(
                &mut ast,
                &module_id,
                &package.package.entry_module,
                &function_index,
                &module_aliases,
            );
            items.extend(ast.items);
        }

        let ir_builder = IrBuilder::default();
        let ir_module = ir_builder.build(&Program { items })?;

        let backend = BytecodeBackend;
        backend.lower_to_bytecode(&ir_module)
    }

    fn frontend_parse(
        &mut self,
        source: &str,
        engine: &mut DiagnosticEngine,
    ) -> Result<Option<Program>, String> {
        let grammar = GrammarEngine::from_registry(&self.language_registry);
        grammar.validate_determinism(&self.language_registry)?;

        let mut lexer = Lexer::from_source(source.to_string());
        let tokens = match lexer.tokenize() {
            Ok(tokens) => tokens,
            Err(e) => {
                engine.emit(e.diagnostic().clone());
                return Ok(None);
            }
        };

        let mut parser = NeuroParser::new(grammar);
        let _ = parser.grammar_size();
        let ast = match parser.parse_incremental(&mut self.incremental_state, source, &tokens) {
            Ok(ast) => ast,
            Err(e) => {
                emit_parser_errors(engine, e);
                return Ok(None);
            }
        };

        let mut ast = ast;
        ProtocolLowerer::lower(&mut ast);

        let semantic = SemanticAnalyzer::new(self.language_registry.types.clone());
        if let Err(message) = semantic.analyze(&ast) {
            engine.emit(
                Diagnostic::error(codes::SEMANTIC_UNDEFINED_SYMBOL, message)
                    .with_suggestion("check symbol definitions and types"),
            );
            return Ok(None);
        }

        Ok(Some(ast))
    }

    pub fn discover_engines(&self) -> Vec<String> {
        self.engine_registry
            .discover()
            .into_iter()
            .map(|e| e.name)
            .collect()
    }

    pub fn compile_package(
        &mut self,
        input: &Path,
        target: &str,
    ) -> Result<crate::runtime::execution::module_loader::NyxPackage, String> {
        let source = fs::read_to_string(input).map_err(|e| e.to_string())?;
        self.check(&source)?;
        Ok(package_entry(input, target).map_err(|e| e.message)?.package)
    }

    pub fn compile_incremental_patch(
        &mut self,
        entry: &Path,
        changed_file: &Path,
        next_version: u64,
    ) -> Result<Vec<ModulePatch>, String> {
        let source = fs::read_to_string(changed_file).map_err(|e| e.to_string())?;
        self.check(&source)?;
        incremental_patch_set(entry, changed_file, next_version).map_err(|e| e.message)
    }
}

type FunctionIndex = std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>;
type ModuleAliasIndex = std::collections::BTreeMap<String, Vec<String>>;

fn emit_parser_errors(engine: &mut DiagnosticEngine, err: ParserErrors) {
    for diag in err.errors {
        engine.emit(diag);
    }
}

fn format_diagnostics(engine: &DiagnosticEngine) -> String {
    let mut out = String::new();
    for diag in &engine.diagnostics {
        out.push_str(&format!("{diag}\n"));
    }
    for err in &engine.nyx_errors {
        out.push_str(&format!("{err}\n"));
    }
    out.trim_end().to_string()
}

fn build_function_index(module_asts: &[(String, Program)], entry_module: &str) -> FunctionIndex {
    let mut out = FunctionIndex::new();
    for (module_id, ast) in module_asts {
        let mut functions = std::collections::BTreeMap::new();
        for item in &ast.items {
            if let ItemKind::Function(f) = &item.kind {
                functions.insert(
                    f.name.clone(),
                    qualified_function_name(module_id, entry_module, &f.name),
                );
            }
        }
        out.insert(module_id.clone(), functions);
    }
    out
}

fn build_module_aliases(
    modules: &[crate::runtime::execution::module_loader::NyxModule],
) -> ModuleAliasIndex {
    let mut aliases = ModuleAliasIndex::new();
    for module in modules {
        for alias in module_aliases_for_id(&module.id) {
            aliases.entry(alias).or_default().push(module.id.clone());
        }
    }

    for matches in aliases.values_mut() {
        matches.sort();
        matches.dedup();
    }

    aliases
}

fn module_aliases_for_id(module_id: &str) -> Vec<String> {
    let mut aliases = vec![module_id.to_string()];
    for prefix in ["src/", "tests/"] {
        if let Some(stripped) = module_id.strip_prefix(prefix) {
            aliases.push(stripped.to_string());
        }
    }

    if let Some(last) = module_id.rsplit('/').next() {
        aliases.push(last.to_string());
        if let Some(stripped) = last.strip_suffix("_unit_tests") {
            aliases.push(format!("{stripped}_tests"));
        }
    }

    let parts: Vec<&str> = module_id.split('/').collect();
    if parts.len() >= 2 {
        aliases.push(parts[parts.len() - 2].to_string());
    }

    aliases.sort();
    aliases.dedup();
    aliases
}

fn qualified_function_name(module_id: &str, entry_module: &str, name: &str) -> String {
    if module_id == entry_module && name == "main" {
        return "main".to_string();
    }

    let prefix = module_id.replace('/', "::");
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}::{name}")
    }
}

fn namespace_module_program(
    ast: &mut Program,
    module_id: &str,
    entry_module: &str,
    function_index: &FunctionIndex,
    module_aliases: &ModuleAliasIndex,
) {
    let imported_modules = imported_modules(ast, module_aliases, module_id);
    let imported_functions = imported_functions(&imported_modules, function_index);
    let local_functions = function_index.get(module_id);

    for item in &mut ast.items {
        if let ItemKind::Function(f) = &mut item.kind {
            namespace_function_body(
                f,
                local_functions,
                &imported_modules,
                &imported_functions,
                module_aliases,
            );
            f.name = qualified_function_name(module_id, entry_module, &f.name);
        }
    }
}

fn imported_modules(
    ast: &Program,
    module_aliases: &ModuleAliasIndex,
    current_module_id: &str,
) -> std::collections::BTreeMap<String, String> {
    let mut bindings = std::collections::BTreeMap::new();
    for item in &ast.items {
        match &item.kind {
            ItemKind::Use(use_decl) => {
                for (path, alias) in flatten_use_tree(&use_decl.tree, &mut Vec::new()) {
                    if let Some(module_id) = resolve_module_alias_in_context(
                        module_aliases,
                        &path.join("/"),
                        current_module_id,
                    ) {
                        let binding = alias.unwrap_or_else(|| {
                            path.last().cloned().unwrap_or_else(|| path.join("::"))
                        });
                        bindings.insert(binding, module_id);
                    }
                }
            }
            ItemKind::Module(ModuleDecl::External(name))
            | ItemKind::Module(ModuleDecl::Inline { name, .. }) => {
                bind_module_name(&mut bindings, module_aliases, current_module_id, name);
            }
            _ => {}
        }
    }
    bindings
}

fn bind_module_name(
    bindings: &mut std::collections::BTreeMap<String, String>,
    module_aliases: &ModuleAliasIndex,
    current_module_id: &str,
    module_name: &str,
) {
    if let Some(module_id) =
        resolve_module_alias_in_context(module_aliases, module_name, current_module_id)
    {
        bindings.insert(module_name.to_string(), module_id.clone());
        if let Some(last) = module_name.rsplit('/').next() {
            bindings.entry(last.to_string()).or_insert(module_id);
        }
    }
}

fn flatten_use_tree(
    tree: &UseTree,
    prefix: &mut Vec<String>,
) -> Vec<(Vec<String>, Option<String>)> {
    match tree {
        UseTree::Path { segment, child } => {
            prefix.push(segment.clone());
            let result = flatten_use_tree(child, prefix);
            prefix.pop();
            result
        }
        UseTree::Group(trees) => {
            let mut result = Vec::new();
            for tree in trees {
                result.extend(flatten_use_tree(tree, prefix));
            }
            result
        }
        UseTree::Name { name, alias } => {
            let mut path = prefix.clone();
            path.push(name.clone());
            vec![(path, alias.clone())]
        }
        UseTree::Glob => vec![],
    }
}

fn resolve_module_alias(module_aliases: &ModuleAliasIndex, alias: &str) -> Option<String> {
    let matches = module_aliases.get(alias)?;
    if matches.len() == 1 {
        matches.first().cloned()
    } else {
        None
    }
}

fn resolve_module_alias_in_context(
    module_aliases: &ModuleAliasIndex,
    alias: &str,
    current_module_id: &str,
) -> Option<String> {
    let matches = module_aliases.get(alias)?;
    if matches.len() == 1 {
        return matches.first().cloned();
    }

    let current_root = current_module_id.split('/').next().unwrap_or_default();
    let mut preferred = matches
        .iter()
        .filter(|module_id| module_id.split('/').next().unwrap_or_default() == current_root)
        .cloned()
        .collect::<Vec<_>>();
    preferred.sort();
    preferred.dedup();

    if preferred.len() == 1 {
        preferred.into_iter().next()
    } else {
        None
    }
}

fn imported_functions(
    imported_modules: &std::collections::BTreeMap<String, String>,
    function_index: &FunctionIndex,
) -> std::collections::BTreeMap<String, String> {
    let mut candidates = std::collections::BTreeMap::<String, Vec<String>>::new();
    for module_id in imported_modules.values() {
        if let Some(functions) = function_index.get(module_id) {
            for (name, qualified) in functions {
                candidates
                    .entry(name.clone())
                    .or_default()
                    .push(qualified.clone());
            }
        }
    }

    let mut resolved = std::collections::BTreeMap::new();
    for (name, matches) in candidates {
        if matches.len() == 1 {
            resolved.insert(name, matches.into_iter().next().unwrap_or_default());
        }
    }
    resolved
}

fn namespace_function_body(
    function: &mut FunctionDecl,
    local_functions: Option<&std::collections::BTreeMap<String, String>>,
    imported_modules: &std::collections::BTreeMap<String, String>,
    imported_functions: &std::collections::BTreeMap<String, String>,
    module_aliases: &ModuleAliasIndex,
) {
    for stmt in &mut function.body {
        rewrite_stmt(
            stmt,
            local_functions,
            imported_modules,
            imported_functions,
            module_aliases,
        );
    }
}

fn rewrite_stmt(
    stmt: &mut Stmt,
    local_functions: Option<&std::collections::BTreeMap<String, String>>,
    imported_modules: &std::collections::BTreeMap<String, String>,
    imported_functions: &std::collections::BTreeMap<String, String>,
    module_aliases: &ModuleAliasIndex,
) {
    match stmt {
        Stmt::Let { expr, .. } => rewrite_expr(
            expr,
            false,
            local_functions,
            imported_modules,
            imported_functions,
            module_aliases,
        ),
        Stmt::Assign { target, value, .. } => {
            rewrite_expr(
                target,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
            rewrite_expr(
                value,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
        }
        Stmt::CompoundAssign { target, value, .. } => {
            rewrite_expr(
                target,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
            rewrite_expr(
                value,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
        }
        Stmt::Return { expr, .. } => {
            if let Some(expr) = expr {
                rewrite_expr(
                    expr,
                    false,
                    local_functions,
                    imported_modules,
                    imported_functions,
                    module_aliases,
                );
            }
        }
        Stmt::Defer { stmt, .. } => {
            rewrite_stmt(
                stmt,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
        }
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for IfBranch { condition, body } in branches {
                rewrite_expr(
                    condition,
                    false,
                    local_functions,
                    imported_modules,
                    imported_functions,
                    module_aliases,
                );
                for stmt in body {
                    rewrite_stmt(
                        stmt,
                        local_functions,
                        imported_modules,
                        imported_functions,
                        module_aliases,
                    );
                }
            }
            if let Some(body) = else_body {
                for stmt in body {
                    rewrite_stmt(
                        stmt,
                        local_functions,
                        imported_modules,
                        imported_functions,
                        module_aliases,
                    );
                }
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            rewrite_expr(
                condition,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
            for stmt in body {
                rewrite_stmt(
                    stmt,
                    local_functions,
                    imported_modules,
                    imported_functions,
                    module_aliases,
                );
            }
        }
        Stmt::ForIn { iter, body, .. } => {
            rewrite_expr(
                iter,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
            for stmt in body {
                rewrite_stmt(
                    stmt,
                    local_functions,
                    imported_modules,
                    imported_functions,
                    module_aliases,
                );
            }
        }
        Stmt::Loop { body, .. } | Stmt::Unsafe { body, .. } => {
            for stmt in body {
                rewrite_stmt(
                    stmt,
                    local_functions,
                    imported_modules,
                    imported_functions,
                    module_aliases,
                );
            }
        }
        Stmt::Match { expr, arms, .. } => {
            rewrite_expr(
                expr,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
            for MatchArm { guard, body, .. } in arms {
                if let Some(guard) = guard {
                    rewrite_expr(
                        guard,
                        false,
                        local_functions,
                        imported_modules,
                        imported_functions,
                        module_aliases,
                    );
                }
                match body {
                    MatchBody::Expr(expr) => {
                        rewrite_expr(
                            expr,
                            false,
                            local_functions,
                            imported_modules,
                            imported_functions,
                            module_aliases,
                        );
                    }
                    MatchBody::Stmt(stmt) => {
                        rewrite_stmt(
                            stmt,
                            local_functions,
                            imported_modules,
                            imported_functions,
                            module_aliases,
                        );
                    }
                    MatchBody::Block(stmts) => {
                        for stmt in stmts {
                            rewrite_stmt(
                                stmt,
                                local_functions,
                                imported_modules,
                                imported_functions,
                                module_aliases,
                            );
                        }
                    }
                }
            }
        }
        Stmt::InlineAsm {
            outputs, inputs, ..
        } => {
            for operand in outputs {
                rewrite_expr(
                    &mut operand.expr,
                    false,
                    local_functions,
                    imported_modules,
                    imported_functions,
                    module_aliases,
                );
            }
            for operand in inputs {
                rewrite_expr(
                    &mut operand.expr,
                    false,
                    local_functions,
                    imported_modules,
                    imported_functions,
                    module_aliases,
                );
            }
        }
        Stmt::Expr(expr) | Stmt::Print { expr } => {
            rewrite_expr(
                expr,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
        }
        Stmt::Break { .. } | Stmt::Continue { .. } => {}
    }
}

fn rewrite_expr(
    expr: &mut Expr,
    is_callee: bool,
    local_functions: Option<&std::collections::BTreeMap<String, String>>,
    imported_modules: &std::collections::BTreeMap<String, String>,
    imported_functions: &std::collections::BTreeMap<String, String>,
    module_aliases: &ModuleAliasIndex,
) {
    match expr {
        Expr::ArrayLiteral(items) | Expr::TupleLiteral(items) => {
            for item in items {
                rewrite_expr(
                    item,
                    false,
                    local_functions,
                    imported_modules,
                    imported_functions,
                    module_aliases,
                );
            }
        }
        Expr::ArrayRepeat { value, len } => {
            rewrite_expr(
                value,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
            rewrite_expr(
                len,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
        }
        Expr::Binary { left, right, .. } => {
            rewrite_expr(
                left,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
            rewrite_expr(
                right,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
        }
        Expr::Unary { right, .. }
        | Expr::TryOp(right)
        | Expr::Await(right)
        | Expr::Deref(right)
        | Expr::Move(right) => {
            rewrite_expr(
                right,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
        }
        Expr::FieldAccess { object, .. } => {
            rewrite_expr(
                object,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
        }
        Expr::MethodCall { receiver, args, .. } => {
            rewrite_expr(
                receiver,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
            for arg in args {
                rewrite_expr(
                    arg,
                    false,
                    local_functions,
                    imported_modules,
                    imported_functions,
                    module_aliases,
                );
            }
        }
        Expr::Index { object, index } => {
            rewrite_expr(
                object,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
            rewrite_expr(
                index,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
        }
        Expr::Slice { object, start, end } => {
            rewrite_expr(
                object,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
            if let Some(start) = start {
                rewrite_expr(
                    start,
                    false,
                    local_functions,
                    imported_modules,
                    imported_functions,
                    module_aliases,
                );
            }
            if let Some(end) = end {
                rewrite_expr(
                    end,
                    false,
                    local_functions,
                    imported_modules,
                    imported_functions,
                    module_aliases,
                );
            }
        }
        Expr::Call { callee, args } => {
            rewrite_expr(
                callee,
                true,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
            for arg in args {
                rewrite_expr(
                    arg,
                    false,
                    local_functions,
                    imported_modules,
                    imported_functions,
                    module_aliases,
                );
            }
        }
        Expr::StructLiteral { fields, .. } => {
            for field in fields {
                rewrite_expr(
                    &mut field.value,
                    false,
                    local_functions,
                    imported_modules,
                    imported_functions,
                    module_aliases,
                );
            }
        }
        Expr::BlockLiteral(items) => {
            for item in items {
                match item {
                    BlockItem::Field(field) => rewrite_expr(
                        &mut field.value,
                        false,
                        local_functions,
                        imported_modules,
                        imported_functions,
                        module_aliases,
                    ),
                    BlockItem::Spread(expr) => rewrite_expr(
                        expr,
                        false,
                        local_functions,
                        imported_modules,
                        imported_functions,
                        module_aliases,
                    ),
                }
            }
        }
        Expr::Cast { expr, .. } => {
            rewrite_expr(
                expr,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
        }
        Expr::Range { start, end, .. } => {
            if let Some(start) = start {
                rewrite_expr(
                    start,
                    false,
                    local_functions,
                    imported_modules,
                    imported_functions,
                    module_aliases,
                );
            }
            if let Some(end) = end {
                rewrite_expr(
                    end,
                    false,
                    local_functions,
                    imported_modules,
                    imported_functions,
                    module_aliases,
                );
            }
        }
        Expr::Closure { body, .. } => {
            rewrite_expr(
                body,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
        }
        Expr::Reference { expr, .. } => {
            rewrite_expr(
                expr,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
        }
        Expr::Block(stmts, tail_expr) => {
            for stmt in stmts {
                rewrite_stmt(
                    stmt,
                    local_functions,
                    imported_modules,
                    imported_functions,
                    module_aliases,
                );
            }
            if let Some(expr) = tail_expr {
                rewrite_expr(
                    expr,
                    false,
                    local_functions,
                    imported_modules,
                    imported_functions,
                    module_aliases,
                );
            }
        }
        Expr::IfExpr {
            branches,
            else_body,
        } => {
            for IfBranch { condition, body } in branches {
                rewrite_expr(
                    condition,
                    false,
                    local_functions,
                    imported_modules,
                    imported_functions,
                    module_aliases,
                );
                for stmt in body {
                    rewrite_stmt(
                        stmt,
                        local_functions,
                        imported_modules,
                        imported_functions,
                        module_aliases,
                    );
                }
            }
            if let Some(expr) = else_body {
                rewrite_expr(
                    expr,
                    false,
                    local_functions,
                    imported_modules,
                    imported_functions,
                    module_aliases,
                );
            }
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            rewrite_expr(
                condition,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
            rewrite_expr(
                then_expr,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
            rewrite_expr(
                else_expr,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
        }
        Expr::Match { expr, arms } => {
            rewrite_expr(
                expr,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
            for MatchArm { guard, body, .. } in arms {
                if let Some(guard) = guard {
                    rewrite_expr(
                        guard,
                        false,
                        local_functions,
                        imported_modules,
                        imported_functions,
                        module_aliases,
                    );
                }
                match body {
                    MatchBody::Expr(expr) => {
                        rewrite_expr(
                            expr,
                            false,
                            local_functions,
                            imported_modules,
                            imported_functions,
                            module_aliases,
                        );
                    }
                    MatchBody::Stmt(stmt) => {
                        rewrite_stmt(
                            stmt,
                            local_functions,
                            imported_modules,
                            imported_functions,
                            module_aliases,
                        );
                    }
                    MatchBody::Block(stmts) => {
                        for stmt in stmts {
                            rewrite_stmt(
                                stmt,
                                local_functions,
                                imported_modules,
                                imported_functions,
                                module_aliases,
                            );
                        }
                    }
                }
            }
        }
        Expr::AsyncBlock(stmts) => {
            for stmt in stmts {
                rewrite_stmt(
                    stmt,
                    local_functions,
                    imported_modules,
                    imported_functions,
                    module_aliases,
                );
            }
        }
        Expr::Loop(expr) => {
            rewrite_expr(
                expr,
                false,
                local_functions,
                imported_modules,
                imported_functions,
                module_aliases,
            );
        }
        Expr::Identifier(name) if is_callee => {
            if let Some(qualified) = local_functions.and_then(|functions| functions.get(name)) {
                *expr = qualified_path_expr(qualified);
            } else if let Some(qualified) = imported_functions.get(name) {
                *expr = qualified_path_expr(qualified);
            }
        }
        Expr::Path(parts) => {
            if let Some(qualified) =
                resolve_path_callee(parts, local_functions, imported_modules, module_aliases)
            {
                *expr = qualified_path_expr(&qualified);
            }
        }
        Expr::IntLiteral(_)
        | Expr::BigIntLiteral(_)
        | Expr::FloatLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::CssLiteral(_)
        | Expr::CharLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NullLiteral
        | Expr::Identifier(_) => {}
    }
}

fn resolve_path_callee(
    parts: &[String],
    local_functions: Option<&std::collections::BTreeMap<String, String>>,
    imported_modules: &std::collections::BTreeMap<String, String>,
    module_aliases: &ModuleAliasIndex,
) -> Option<String> {
    if parts.is_empty() {
        return None;
    }

    let joined = parts.join("::");
    if let Some(local_functions) = local_functions {
        if let Some(qualified) = local_functions.get(&joined) {
            return Some(qualified.clone());
        }
    }

    if parts.len() < 2 {
        return None;
    }

    let function_name = parts.last()?.clone();
    let module_alias = parts[..parts.len() - 1].join("/");
    let module_id = imported_modules
        .get(&module_alias)
        .cloned()
        .or_else(|| resolve_module_alias(module_aliases, &module_alias))?;
    Some(format!(
        "{}::{}",
        module_id.replace('/', "::"),
        function_name
    ))
}

fn qualified_path_expr(qualified: &str) -> Expr {
    Expr::Path(qualified.split("::").map(|part| part.to_string()).collect())
}

fn get_nyx_home() -> PathBuf {
    if let Ok(home) = std::env::var("NYX_HOME") {
        return PathBuf::from(home);
    }
    PathBuf::from("/home/surya/Nyx Programming Language")
}

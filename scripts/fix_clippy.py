import os

def replace_in_file(path, old, new):
    if not os.path.exists(path): return
    with open(path, "r") as f: content = f.read()
    content = content.replace(old, new)
    with open(path, "w") as f: f.write(content)

# 1. ast_nodes.rs - empty line after doc comment
replace_in_file("src/core/ast/ast_nodes.rs", 
"""/// Old StructDecl shape: the `fields: Vec<Field>` field is preserved for layout tests.

pub type ParamCompat = Param;""",
"""/// Old StructDecl shape: the `fields: Vec<Field>` field is preserved for layout tests.
pub type ParamCompat = Param;""")

# 2. unsafe_ops.rs - dead code
replace_in_file("src/systems/hardware/unsafe_ops.rs", "pub struct UnsafeCell<T> {", "#[allow(dead_code)]\npub struct UnsafeCell<T> {")

# 3. io.rs - dead code
replace_in_file("src/systems/hardware/io.rs", "pub struct IoPort<T: Copy> {", "#[allow(dead_code)]\npub struct IoPort<T: Copy> {")

# 4. lexer/mod.rs - module inception
replace_in_file("src/core/lexer/mod.rs", "pub mod lexer;", "#![allow(clippy::module_inception)]\npub mod lexer;")

# 5. neuro_parser.rs - manual strip
replace_in_file("src/core/parser/neuro_parser.rs",
"""                } else if v.starts_with("0b") {
                    i64::from_str_radix(&v[2..], 2).unwrap_or(0)
                } else if v.starts_with("0o") {
                    i64::from_str_radix(&v[2..], 8).unwrap_or(0)""",
"""                } else if let Some(stripped) = v.strip_prefix("0b") {
                    i64::from_str_radix(stripped, 2).unwrap_or(0)
                } else if let Some(stripped) = v.strip_prefix("0o") {
                    i64::from_str_radix(stripped, 8).unwrap_or(0)""")

# 6. type_system.rs - should implement trait
replace_in_file("src/core/semantic/type_system.rs", "pub fn from_str(s: &str) -> Self {", "#[allow(clippy::should_implement_trait)]\n    pub fn from_str(s: &str) -> Self {")

# 7. ir_builder.rs - only used in recursion
replace_in_file("src/systems/ir/ir_builder.rs", "fn lower_expr(", "#[allow(clippy::only_used_in_recursion)]\n    fn lower_expr(")


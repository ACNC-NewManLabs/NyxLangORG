#!/bin/bash
set -e

cd /home/surya/NPL/nyx/src

mkdir -p core runtime systems extensions applications

# 1. Move to CORE
mv ast lexer parser semantic registry diagnostics.rs core/

# 2. Move to SYSTEMS
# existing systems directory contains files, let's reorganize it
mkdir -p systems/hardware
mv systems/*.rs systems/hardware/ 2>/dev/null || true
mv backend ir systems/

# 3. Move to APPLICATIONS
mv compiler applications/

# 4. Handle RUNTIME and EXTENSIONS
# Create empty module files or default structures
cat << 'EOF' > runtime/mod.rs
//! Runtime Layer: Execution multi-threading and secure sandbox.
pub mod execution;
pub mod sandbox;
EOF

mkdir -p runtime/execution runtime/sandbox
touch runtime/execution/mod.rs runtime/sandbox/mod.rs

cat << 'EOF' > extensions/mod.rs
//! Extensions Layer: Plugin loading and domain-specific extensions.
pub mod plugin_loader;
pub mod interfaces;
EOF

mkdir -p extensions/plugin_loader extensions/interfaces
touch extensions/plugin_loader/mod.rs extensions/interfaces/mod.rs

# 5. Fix references using find & sed
# We need to replace `crate::ast`, `crate::lexer`, `crate::parser`, `crate::semantic`, `crate::registry`, `crate::diagnostics` with `crate::core::...`
# We need to replace `crate::backend`, `crate::ir` with `crate::systems::...`
# We need to replace `crate::systems::` (old hardware ones) with `crate::systems::hardware::` unless it's just crate::systems
# We need to replace `crate::compiler` with `crate::applications::compiler`

find . -type f -name "*.rs" -print0 | xargs -0 sed -i 's/crate::ast/crate::core::ast/g'
find . -type f -name "*.rs" -print0 | xargs -0 sed -i 's/crate::lexer/crate::core::lexer/g'
find . -type f -name "*.rs" -print0 | xargs -0 sed -i 's/crate::parser/crate::core::parser/g'
find . -type f -name "*.rs" -print0 | xargs -0 sed -i 's/crate::semantic/crate::core::semantic/g'
find . -type f -name "*.rs" -print0 | xargs -0 sed -i 's/crate::registry/crate::core::registry/g'
find . -type f -name "*.rs" -print0 | xargs -0 sed -i 's/crate::diagnostics/crate::core::diagnostics/g'

# Handle older `crate::systems::` calls -> `crate::systems::hardware::`
find . -type f -name "*.rs" -print0 | xargs -0 sed -i 's/crate::systems::/crate::systems::hardware::/g'

find . -type f -name "*.rs" -print0 | xargs -0 sed -i 's/crate::backend/crate::systems::backend/g'
find . -type f -name "*.rs" -print0 | xargs -0 sed -i 's/crate::ir/crate::systems::ir/g'

find . -type f -name "*.rs" -print0 | xargs -0 sed -i 's/crate::compiler/crate::applications::compiler/g'

# 6. Rebuild lib.rs
cat << 'EOF' > lib.rs
//! Nyx Universal Core Architecture
//! 
//! Layered architecture:
//! - core: Domain-agnostic compiler core (lexer, parser, ast, sema)
//! - runtime: Virtual execution, sandbox, concurrency
//! - systems: Low-level IR, LLVM backends, hardware interfaces
//! - extensions: Plugins and domain-specific features
//! - applications: CLI, UI Runner, overall tools

pub mod core;
pub mod runtime;
pub mod systems;
pub mod extensions;
pub mod applications;
EOF

# Create mod.rs for directories
cat << 'EOF' > core/mod.rs
pub mod ast;
pub mod lexer;
pub mod parser;
pub mod semantic;
pub mod registry;
pub mod diagnostics;
EOF

cat << 'EOF' > systems/mod.rs
pub mod backend;
pub mod ir;
pub mod hardware;
EOF

cat << 'EOF' > applications/mod.rs
pub mod compiler;
EOF

echo "Done"

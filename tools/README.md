# 🌌 Nyx Language: The Advanced Toolchain™

**Copyright (c) 2026 SURYA SEKHAR ROY. All Rights Reserved.**
*Nyx™, Nyx Nexus™, and the Nyx Logo are trademarks of SURYA SEKHAR ROY.*
*This software is protected under a strict Proprietary License. See [LICENSE](./LICENSE) for full legal terms.*

Welcome to the state-of-the-art developer experience for the Nyx™ Language. This suite is engineered for extreme performance, transparency, and production reliability.

## 🚀 The Universal CLI

The `nyx` binary is the high-performance dispatcher for all language tasks.

```bash
./tools/nyx [COMMAND] [ARGS]
```

### 💎 Command Portfolio

| Command | Category | Description |
| :--- | :--- | :--- |
| `format` | **Polish** | **Nyx Formatter™**: AST-based code formatter for consistent styling. |
| `lint` | **Quality** | **Nyx Linter™**: Semantic linter for structural auditing and safety. |
| `debug` | **Runtime** | **Nyx Debugger™**: Interactive VM debugger with stack inspection. |
| `profile` | **Perf** | **Nyx Profiler™**: Execution sampling and function-level bottlenecks. |
| `bench` | **Perf** | **Nyx Bench™**: Deterministic benchmarking with instruction counting. |
| `flow` | **Analyze** | **Nyx Flow™**: Visual call graph generation (Mermaid). |
| `repl` | **Exploration** | **Nyx REPL™**: Interactive REPL with real-time VM evaluation. |
| `cat-bc` | **Transparency** | **Nyx Disassembler™**: Disassemble Nyx modules into human-readable bytecode. |
| `ast` | **Architecture** | **Nyx AST Explorer™**: Explore the Abstract Syntax Tree (AST) in JSON/Text. |
| `lsp` | **Editor** | **Nyx LSP™**: Language Server Protocol for IDE integration. |
| `security` | **Safety** | **Nyx Security™**: Static security scanner for vulnerability detection. |
| `doctor` | **Health** | **Nyx Doctor™**: System and ecosystem environment audit. |
| `tune` | **Auto** | **Nyx Tune™**: Autonomous optimization and ecosystem tuning. |
| `nexus` | **Executive** | **Nyx Nexus™**: High-fidelity local web dashboard (The Nexus). |
| `ai` | **Zen** | **Nyx AI™**: AI-native generation, optimization, and automated fixes. |

---

## 🛠 Apex Deep-Dive

### 💅 Nyx Formatter (`nyx format`) — Production-Grade Source Formatter
> **Build Status:** ✅ `nyx_formatter v1.0.0` — Zero errors, zero warnings.

A fast, deterministic, token-stream formatter for `.nyx` source files. Uses the shared Nyx lexer so it can never drift out of sync with the language grammar.

#### Usage

```bash
# Format a single file in-place
nyx format myfile.nyx

# Format all .nyx files in the current directory (recursive)
nyx format .

# Format multiple paths at once
nyx format src/ lib/ main.nyx

# Check mode: report unformatted files, exit 1 if any found (CI-safe)
nyx format --check .

# Show a unified diff without writing anything
nyx format --diff myfile.nyx

# Custom indentation (default: 4)
nyx format --indent 2 .

# Suppress output
nyx format --quiet .

# Verbose: show every file being considered
nyx format --verbose .
```

#### CLI Options

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--check` | `-c` | off | Report unformatted files; exit 1 if any found |
| `--diff` | `-d` | off | Show unified diff without writing |
| `--indent N` | | `4` | Indentation width in spaces |
| `--max-line N` | | `100` | Soft line-length limit (reserved for future wrapping) |
| `--quiet` | `-q` | off | Suppress all output except errors |
| `--verbose` | `-v` | off | Print every file being examined |

#### Formatting Rules

| Rule | Description |
| :--- | :--- |
| **Indentation** | 4-space (configurable) indent on open brace `{` |
| **Operators** | Spaces around all binary operators (`=`, `+`, `==`, `&&`, etc.) |
| **Function calls** | No space between identifier and `(` — e.g., `foo(x)` not `foo (x)` |
| **Commas** | Space after `,` but not before |
| **Semicolons** | No space before `;`; newline after |
| **Comments** | Single-line and block comments each get their own line |
| **Blank lines** | At most one blank line between top-level items |
| **Final newline** | All files end with exactly one `\n` |
| **Directories** | Recursively scans; skips `target/`, `node_modules/`, `.git/` |

#### Exit Codes

| Code | Meaning |
| :--- | :--- |
| `0` | All files formatted (or already up to date) |
| `1` | `--check` mode: unformatted files found |
| `2` | I/O or parse error on one or more files |

#### Architecture

The formatter is built on the **shared Nyx lexer** (`Lexer::from_source`), so its understanding of tokens is identical to the compiler's. The format pass is a single linear scan over the token stream, tracking:
- `indent_level` — increments on `{`, decrements before `}`
- `at_line_start` — controls when indentation is emitted
- `blank_lines` — collapses consecutive blank lines to at most 1


### 🗺️ Nyx Nexus (`nyx nexus`) — Production-Grade Executive Dashboard
> **Build Status:** ✅ `nyx-nexus v1.0.0` — Zero errors, zero warnings.

The centerpiece of the Apex Suite. A terminal-launched, high-fidelity web dashboard providing real-time project intelligence, health audits, and ecosystem metrics — all served from a WebSocket-enabled Axum backend.

#### Launch

```bash
# From project root (opens browser automatically)
cargo run -p nyx-nexus -- --root .

# Full options
cargo run -p nyx-nexus -- --port 4000 --root /path/to/project --no-open --verbose
```

#### Dashboard Panels

| Panel | Description |
| :--- | :--- |
| **Overview** | Health score (98%), file counts, lines of code, VM heartbeat with live CPU/RAM bars |
| **Health Audit** | Automated checks for Nyx CLI, Cargo, KVM, Language Registry, stdlib, VM crate |
| **Diagnostics** | Real-time `.nyx` source linting — detects TODOs, style violations. Filterable by type. |
| **Architecture** | Canvas-based circular module dependency graph built from live directory structure |
| **File Explorer** | Searchable, extension-colored file tree sorted by last-modified date |
| **System Vitals** | Live CPU history chart (60-point canvas), RAM %, Disk usage, Load Average |

#### Backend API Endpoints

| Endpoint | Method | Description |
| :--- | :--- | :--- |
| `/api/status` | `GET` | Project name, health score, file counts, LoC, uptime |
| `/api/health` | `GET` | Ecosystem health checks with pass/fail and category |
| `/api/vitals` | `GET` | CPU %, RAM MB, Disk GB, load average, process count |
| `/api/diagnostics` | `GET` | Real source-scanned linting diagnostics |
| `/api/files` | `GET` | Top 20 files sorted by modified time |
| `/api/modules` | `GET` | Module graph (nodes + edges) from directory structure |
| `/ws` | `WebSocket` | Real-time vitals broadcast every 2 seconds |

#### Architecture Highlights

- **WebSocket Streaming** — `tokio::sync::broadcast` channel pushes vitals to all connected clients every 2 seconds
- **`AppState`** — Thread-safe `Arc<AppState>` holds project root and event channel
- **Structured Logging** — `tracing` / `tracing-subscriber` with timestamps and log levels
- **Zero Panics** — All I/O operations use `filter_map` and `.ok()` fallbacks
- **Smart UI Serving** — Detects UI path from multiple locations for flexible launch directory

---

### 🐛 Nyx Debugger (`nyx debug`) — Production-Grade Interactive Debugger
> **Build Status:** ✅ `nyx-debugger v0.1.0` — 7/7 tests pass.

A fully integrated interactive CLI debugger that hooks into the Nyx VM's `on_step` callback for source-level control.

#### Usage

```bash
nyx debug run <file.nyx>
```

#### Commands (while paused)

| Command | Description |
| :--- | :--- |
| `c` / `continue` | Resume execution |
| `s` / `step` | Step to next instruction |
| `bt` / `backtrace` | Print call stack frames |
| `l` / `locals` | Inspect local scope and stack top |
| `src` / `list` | Show source context with current-line marker (`►`) |
| `b <N>` | Set breakpoint at line N |
| `h` / `help` | Show all commands |
| `q` / `quit` | Exit session |

#### Architecture Highlights

- **`BreakpointManager`** — Tracks file-level line breakpoints, enable/disable per ID
- **`VariableInspector`** — Stateful variable capture and expression evaluation
- **`DebugRuntime`** — Tracks session state (`Running`, `Stepping`, `Paused`) and call history
- **`on_step` Hook** — Integrates directly with `VmConfig` for instruction-granular control

---

### 🧠 Nyx AI Fix (`nyx ai fix`)
Automated self-healing for your codebase. By piping compiler diagnostics directly into the AI engine, Nyx can solve its own compilation errors and suggest structural optimizations.

### ⏱️ Deterministic Benchmarking (`nyx bench`)
Unlike jitter-prone wall-clock benchmarks, Nyx measures execution cost via exact VM instruction counts and memory allocation tracking. Your benchmarks yield identical results across different hardware.

---

## 🏗 Architectural Philosophy

The Nyx toolset follows a **Unified Frontend** philosophy. Unlike other languages that use separate parsers for formatting, linting, and compilation, Nyx tools all leverage the **Shared Core Compiler**:

1. **Shared Lexer/Parser**: Guaranteed consistency across all tools.
2. **VM-Native Tooling**: Debuggers and Profilers are built *into* the VM hooks, not on top of them.
3. **Production First**: Every tool uses the actual production engine, not a mock implementation.
4. **Deterministic Auditing**: Hardware-independent performance metrics at the VM level.

---

*Nyx Tooling: Engineering the future of developer efficiency.*

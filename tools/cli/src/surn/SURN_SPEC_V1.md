# SURN: Structured Universal Runtime Notation
## Formal Technical Standard v1.0

---

## 1. Introduction

### 1.1 Overview
**SURN (Structured Universal Runtime Notation)** is an industrial-grade, universal configuration and data serialization format designed to unify the strengths of established standards while eliminating their inherent ambiguities and performance bottlenecks. SURN provides a deterministic, type-safe, and human-optimized syntax suitable for modern systems programming, large-scale infrastructure, and high-performance API communications.

### 1.2 Motivation
The modern software ecosystem is fragmented across formats that each excel in narrow domains but fail as universal solutions:
- **TOML** is excellent for flat configuration but becomes unwieldy for deep hierarchy.
- **JSON** is the standard for data exchange but lacks readability for configuration and supports no comments.
- **YAML** offers beautiful hierarchy but suffers from significant grammar ambiguity (the "Norway problem"), slow parsing, and complex specification.

SURN was created to solve these problems by providing a single, coherent grammar that supports **Configuration Tables**, **Structured Objects**, and **Hierarchical Blocks** without ambiguity.

### 1.3 Philosophy
SURN's design philosophy is centered on:
- **Explicitness**: No hidden behavior or "magic" type coercion.
- **Determinism**: The same document always parses to the same AST regardless of the implementation.
- **Human-Centricity**: Syntax that prioritizes readability and ease of manual editing.
- **Machine-Efficiency**: A grammar optimized for O(n) single-pass parsing with minimal lookahead.

### 1.4 Use Cases
- **Compiler Packaging**: Manifests for languages (e.g., Nyx's `load.surn`).
- **Cloud Infrastructure**: Deterministic definitions for VPCs, clusters, and services.
- **Distributed Systems**: High-bandwidth API payloads.
- **Build Systems**: Complex task graphs and dependency trees.

---

## 2. Design Principles

### 2.1 Human Readability
SURN utilizes a clean, minimal-noise syntax. It prioritizes identifiers and values over brackets where possible, while maintaining strict delimiters where it aids clarity.

### 2.2 Deterministic Parsing
Unlike formats with complex "implicit types," SURN requires explicit notation for ambiguous cases. A parser must never "guess" the intent of the author.

### 2.3 Performance-Oriented
The grammar is designed for SIMD-accelerated lexing and recursive descent parsing. The elimination of back-tracking ensures that SURN can handle files in the gigabyte range with linear time complexity.

### 2.4 Structural Integrity
SURN enforces a strict hierarchy. Mixing indentation styles (tabs vs. spaces) is a syntax error, preventing the subtle bugs common in YAML-based infrastructure.

---

## 3. File Structure

### 3.1 Document Organization
A `.surn` file consists of a UTF-8 encoded stream of tokens organized into **Tables**, **Assignments**, and **Blocks**.

### 3.2 Global Scope
At the root of the document, assignments and blocks exist in the global scope until a Table declaration is encountered.

### 3.3 Examples

#### Project Configuration (`load.surn`)
```surn
[package]
name = "nyx-core"
version = "1.2.0"
edition = "2024"

dependencies:
    stdlib = "1.0"
    crypto = { git = "...", version = "2.1" }

[profile.release]
opt-level = 3
debug = false
```

#### Infrastructure Block
```surn
network:
    vpc_id: "vpc-0a1b2c3d"
    subnets:
        - "10.0.1.0/24"
        - "10.0.2.0/24"
    security_groups:
        - name: "web-sg"
          rules:
              - port: 80
                allow: "0.0.0.0/0"
```

---

## 4. Syntax Specification

### 4.1 Key-Value Declarations
The fundamental unit of SURN is the assignment.
```surn
key = value   # Standard assignment
key: value    # Inline pair (used in Objects and Blocks)
```

### 4.2 Tables
Tables start with `[` and end with `]`. They define a namespace for subsequent assignments.
```surn
[database.connection]
host = "localhost"
port = 5432
```

### 4.3 Objects
Objects are comma-separated pairs enclosed in `{}`.
```surn
settings = { theme: "dark", notifications: true }
```

### 4.4 Blocks
Blocks use indentation for hierarchy. A key followed by a colon and a newline initiates a block.
```surn
compute:
    instance_type = "t3.medium"
    count = 5
```

---

## 5. Formal Grammar (EBNF)

### 5.1 Lexical Tokens
```ebnf
L_IDENT      = [a-zA-Z_] { [a-zA-Z0-9_\-] } ;
L_STRING     = '"' { L_CHAR } '"' | "'" { L_CHAR } "'" ;
L_INTEGER    = [+-] ? [0-9]+ { "_" [0-9]+ } ;
L_FLOAT      = [+-] ? [0-9]+ "." [0-9]+ [ [eE] [+-] ? [0-9]+ ] ;
L_BOOL       = "true" | "false" ;
L_NULL       = "null" ;
```

### 5.2 Syntactic Rules
```ebnf
Document      = { Statement } ;
Statement     = Table | Assignment | Block ;
Table         = "[" L_IDENT { "." L_IDENT } "]" Newline { Assignment } ;
Assignment    = L_IDENT ( "=" | ":" ) Value [ Newline | "," ] ;
Block         = L_IDENT ":" Newline Indent { Statement } Dedent ;
Value         = L_STRING | L_INTEGER | L_FLOAT | L_BOOL | L_NULL | Array | Object | Block ;
Array         = "[" [ Value { "," Value } ] "]" ;
Object        = "{" [ Assignment { "," Assignment } ] "}" ;
```

---

## 6. Type System

| Type | Syntax | Internal Representation |
|---|---|---|
| **String** | `"..."` | UTF-8 String |
| **Integer** | `123`, `1_000` | 64-bit Signed |
| **Float** | `3.14` | 64-bit IEEE 754 |
| **Boolean** | `true`, `false` | Boolean |
| **Null** | `null` | Unit/Option::None |
| **Array** | `[...]` | Ordered List |
| **Object** | `{...}` | Hash Map / Record |
| **DateTime** | `2024-03-14` | ISO 8601 String |
| **Binary** | `base64(...)` | Byte Array |

---

## 7. Parser Architecture

### 7.1 Lexing
The lexer must be **State-Aware**. It tracks the `IndentStack` to emit virtual `Indent` and `Dedent` tokens. This ensures the parser remains a standard recursive descent parser without needing to check whitespace context in every rule.

### 7.2 Parsing Strategy
Implementations should use **Lookahead-2**.
1. Peek 1: Check token kind (e.g., `Identifier`).
2. Peek 2: Check for `=` (Assignment) or `:` (Block).

### 7.3 Implementation Guidance
- **Rust**: Use `Peekable<Chars>` and a custom `Vec<usize>` for indentation levels.
- **Go**: Use `text/scanner` style streaming lexing.
- **C++**: Utilize `std::string_view` for zero-copy parsing.

---

## 8. Abstract Syntax Tree (AST)

An compliant SURN implementation must provide an AST with the following nodes:
- `Document(Vec<Statement>)`
- `Table(header: Vec<String>, assignments: Vec<Assignment>)`
- `Assignment(key: String, value: Value)`
- `Block(key: String, children: Vec<Statement>)`
- `Value(Primitive | Object | Array)`

---

## 9. Command Line Tooling

The official `surn` binary provides the following management capability:

```bash
surn validate config.surn    # Verifies syntax and structure
surn format config.surn      # Enforces canonical 4-space formatting
surn lint config.surn        # Checks for best practices
surn convert config.surn --to json  # Lossless format conversion
```

---

## 10. Schema System (`surn-schema`)

SURN schemas allow for strict validation of content.
```surn
# schema.surn
[package]
name:
    type = "string"
    pattern = "^[a-z\-]+$"
version:
    type = "string"
    required = true
```

---

## 11. Conversion System

### 11.1 SURN to JSON
- Tables are mapped to top-level object keys.
- Blocks are mapped to nested objects.
- Comments are discarded.

### 11.2 SURN to TOML
- Deeply nested objects in SURN are converted to `[parent.child]` table headers in TOML.

---

## 12. Performance Characteristics

SURN targets the following metrics on a modern workstation (e.g., Apple M2 or AMD Ryzen 9):
- **1,000,000 lines**: < 2.0 seconds.
- **Memory Overhead**: < 2x the raw file size for the AST.
- **Streaming**: Supports SAX-like streaming to process 10GB+ files without loading into memory.

---

## 13. Tooling Ecosystem

- **LSP**: `surn-language-server` for VS Code, Neovim, and JetBrains.
- **Formatter**: `surn-fmt` for CI/CD pipeline enforcement.
- **WASM**: A `surn-parser` compiled to WASM for browser-based configuration editors.

---

## 14. Real-World Examples

### 14.1 Microservice Deployment
```surn
service:
    name = "auth-provider"
    runtime = "nyx-vm"
    resources:
        cpu = 2
        memory = "4GB"
    env:
        DB_URL = "postgres://..."
        SECRET_KEY = { from_vault: "auth/secrets" }
```

---

## 15. Security Considerations

- **Denial of Service (DoS)**: Parsers must limit recursion depth for deeply nested blocks or objects to prevent stack overflow.
- **Size Limits**: Enforce maximum identifier length and string size.
- **Comment Injection**: Ensure serializers correctly escape special characters to prevent comment breaks.

---

## 16. Versioning System

Every compliant SURN file may optionally declare its specification version:
```surn
surn_version = "1.0"
```
Minor versions are backwards compatible. Major versions indicate breaking grammar changes.

---

## 17. License

**The SURN Specification License**
Copyright (c) 2024 SURN Development Team.

Permission is hereby granted, free of charge, to any person obtaining a copy of this specification and associated documentation files, to deal in the Specification without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Specification...

THE SPECIFICATION IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND...

---

## 18. Trademark Declaration

**SURN™ (Structured Universal Runtime Notation)** is a protected trademark. Usage of the name and logo in association with software libraries, tools, and services is permitted provided the implementation adheres to the 100% compliance test suite.

---

## 19. Governance Model

SURN is governed by the **SURN Technical Steering Committee (TSC)**.
- **SPC Process**: SURN Proposal for Change (similar to PEPs or RFCs).
- **TSC Vote**: Required for major version bumps.

---

## 20. Future Roadmap

- **SURN-Bin**: A binary representation (similar to MessagePack/CBOR) for wire transmission.
- **Schema-Gen**: Tooling to generate Rust/Go structs directly from `.surn` files.
- **Native JIT Parsing**: Experimental parser using JIT compilation for maximum throughput.

# Nyx VM Blue Book

This document is the full technical reference for the Nyx VM module under `nyx/vm/`. It is intended to be the definitive, implementation‑accurate “blue book” for developers, auditors, and engine/runtime integrators.

Contents
1. Overview
2. Module Layout
3. Bytecode Model
4. Instruction Set
5. Value Model
6. Function and Module Model
7. Runtime Architecture
8. Heap and Garbage Collection
9. Execution Limits and Safety
10. Native Calls
11. Error Model
12. Determinism and Portability
13. Testing and Validation
14. Embedding Guide
15. Known Limitations and Future Work

--------------------------------------------------------------------------------

## 1. Overview
The Nyx VM is a stack‑based bytecode virtual machine designed to execute Nyx programs. It provides a compact instruction set, a runtime with call frames, a heap with optional garbage collection, and a native function bridge for host integration. The VM is implemented in Rust and is meant to be portable and deterministic across supported targets.

Key design goals:
- Predictable execution model with explicit control‑flow and operand validation.
- Bounded resource usage via stack size, call depth, heap size, and execution limits.
- Clean embedding API for tools and runtime environments.
- Clear error model and explicit failure modes.

--------------------------------------------------------------------------------

## 2. Module Layout
Directory: `nyx/vm/`

Primary source files:
- `src/lib.rs`: VM public API, config, error types, and module exports.
- `src/bytecode.rs`: Bytecode instruction set, value model, module format.
- `src/runtime.rs`: VM runtime loop, stack frames, heap, GC, opcode execution.
- `src/jit.rs`: Cranelift JIT tiers (numeric fast-path and VM-aware stub JIT).
- `src/emitter.rs`: Bytecode emitter helpers and small samples.
- `src/loader.rs`: Bytecode format loading utilities.

Crate name: `nyx-vm`

--------------------------------------------------------------------------------

## 3. Bytecode Model
The VM executes a sequence of `BytecodeInstr` instructions. Each instruction carries:
- `opcode`: the instruction class (`OpCode`).
- `operands`: a vector of signed 32‑bit integers (`Vec<i32>`).
- `line`: a line number for debugging and diagnostics.

Opcodes are fixed to 71 entries and are enumerated as `OpCode` with a stable `repr(u8)` mapping.

Bytecode version:
- Current bytecode version is `6`.
- Version 5 extended closure value serialization with upvalue debug metadata (upvalue names).
- Version 6 extends closure value serialization with upvalue capture metadata (`upvalue_captures`), including by-value vs by-reference (local vs stack) capture details.

Execution model:
- Stack‑based with explicit pushes and pops.
- Locals and arguments are stored in the stack area owned by each call frame.
- Control‑flow uses absolute instruction indices within a function’s instruction list.

--------------------------------------------------------------------------------

## 4. Instruction Set

Control Flow
- `HALT`: Stop execution and return `Unit`.
- `NOP`: No operation.
- `CALL func_idx, argc`: Call a VM function by index.
- `CALL -1, argc`: Dynamic call; pops callee from the stack (callee must be on top, args below).
- `RET`: Return from current frame.
- `JMP target`: Jump unconditionally.
- `JZ target`: Pop condition; jump if falsey.
- `JNZ target`: Pop condition; jump if truthy.
- `CallExt name_idx, argc`: Call a host native by name.

Stack Operations
- `PUSH const_idx`: Push constant from current function’s constant pool.
- `PUSHM const_idx`: Push constant from the module constant pool.
- `POP`: Pop top value.
- `DUP`: Duplicate top value.
- `DUP2`: Duplicate top two values.
- `SWAP`: Swap top two values.
- `ROT`: Rotate top three values.
- `PICK n`: Copy the nth value from the top (0 = top) and push it.
- `PUT n`: Replace the nth value from the top with the top value.

Arithmetic
- `ADD`, `SUB`, `MUL`, `DIV`, `MOD`.
- `NEG`, `INC`, `DEC`, `POW`.
- `DivRem`: Integer division and remainder, pushes both.
- `ABS`, `MIN`, `MAX`.

Comparison
- `CMP`: Returns -1, 0, 1 for less, equal, greater.
- `EQ`, `NE`, `LT`, `GT`, `LE`, `GE`.
- `IsNull`: True if value is `Null`.

Logical
- `AND`, `OR`, `NOT`, `XOR` over truthiness.

Bitwise
- `BAND`, `BOR`, `BNOT`, `BXOR`, `SHL`, `SHR`, `USHR` over integer values.

Memory
- `LOAD local_idx`: Load local from frame slot.
- `STORE local_idx`: Store to local frame slot.
- `ALLOC size_hint`: Allocate value on heap and push pointer.
- `FREE`: Free heap pointer.
- `GetGlobal name_idx`: Get global by name.
- `SetGlobal name_idx`: Set global by name.
- `GetGlobalM name_idx`: Get global by name where the name is a module constant string.
- `SetGlobalM name_idx`: Set global by name where the name is a module constant string.

Object/Array
- `NewArray len`: Pop `len` items and create array.
- `NewObj pairs`: Pop `pairs` key/value pairs (string key) and create object.
- `GetField`: Pop field name and target, push value.
- `SetField`: Pop value, field name, target, set field.
- `GetIndex`: Pop index and target, push value.
- `SetIndex`: Pop value, index, target, set array entry.
- `LEN`: Pop value, push length.
- `SLICE`: Pop end, start, target, push slice.

Closure
- `CLOSURE func_idx, upvalue_count`: Pop `upvalue_count` values, capture them, and push a closure.
- `CLOSURE_REF func_idx, upvalue_count, ...local_idxs`: Capture locals by reference and push a closure.
- `CLOSURE_REF_STACK func_idx, upvalue_count`: Capture `upvalue_count` stack values by reference and push a closure.

All operands are validated. Negative operands for indices or lengths are rejected with `InvalidOperand` errors.

--------------------------------------------------------------------------------

## 5. Value Model
The VM uses the `Value` enum for runtime values:
- `Null` and `Unit`.
- `Bool`, `Int` (i64), `Float` (f64).
- `String`.
- `Array(Vec<Value>)`.
- `Object(HashMap<String, Value>)`.
- `Function(usize)`.
- `NativeFunc(NativeFunction)`.
- `Closure(Closure)`.
- `Pointer(usize)` for heap references.

Truthiness:
- `Null` and `Unit` are falsey.
- `Bool` uses its value.
- `Int` and `Float` are falsey if zero.
- `String` and `Array` and `Object` are falsey if empty.
- `Function`, `NativeFunc`, `Closure`, `Pointer` are truthy (pointer is falsey only if 0).

--------------------------------------------------------------------------------

## 6. Function and Module Model

Function
- `name`: string identifier.
- `arity`: number of arguments required.
- `num_locals`: number of local slots (includes arguments).
- `instructions`: bytecode for the function.
- `constants`: per‑function constant pool.
- `upvalues`: closure metadata.
- `line_info`: instruction to source line mapping.

Module
- `name`: module name.
- `functions`: vector of `Function`.
- `globals`: vector of global variable names.
- `constants`: module‑level constant pool.
- `source_path`, `dependencies`.

The runtime uses function-level constants for `PUSH`, `GetGlobal`, and `SetGlobal`. Module-level constants are available via `PUSHM` and via constant fallback where supported; module-constant global names are supported via `GetGlobalM` and `SetGlobalM`.

--------------------------------------------------------------------------------

## 7. Runtime Architecture
The runtime is `VmRuntime` and consists of:
- `frames`: call stack of `Frame`.
- `stack`: operand stack with configurable max size.
- `globals`: global symbol table.
- `modules`: loaded bytecode modules.
- `natives`: registered host functions.
- `jit_engine`: optional Cranelift JIT engine (enabled by `VmConfig.enable_jit` + `jit` feature).
- `heap`: heap storage for pointer‑managed values.
- `instruction_count`: count of executed instructions.
- `start_time`: execution start time for time limits.
- `call_depth`: active call depth count.

Frame contents:
- `module_name`: name of module for the frame.
- `function`: the executing `Function`.
- `ip`: instruction pointer within `function.instructions`.
- `stack_base`: base index of frame locals.
- `num_locals`: local slots for the frame.

Call model:
- Arguments are pushed onto the stack before a `CALL`.
- For `CALL -1, argc`, the callee must be on top of stack with the arguments beneath it.
- A new frame’s locals are extended up to `num_locals` with `Null`.
- Return value is pushed onto the caller’s stack.

JIT execution (Cranelift)

The VM has two JIT tiers, both behind the `jit` Cargo feature and `VmConfig.enable_jit`:

1) Numeric JIT (typed, direct-call)
- Compiles certain functions into typed machine code with signature `(...args) -> (i64|f64)`.
- Only applies when a conservative `JitPlan` can be inferred (constants are numeric-only, straight-line stack simulation succeeds, single `RET`, etc.).
- Arity is currently limited to 32 (direct ABI-safe calls only).

2) VM-aware stub JIT (full `Value` semantics via runtime stubs)
- The VM-aware stub JIT supports **Inline Cache (IC) inlining** for `GetField`. For constant-keyed object property access, the generated code validates the cache directly (checking target, generation, and version) via specialized stubs, falling back to a full runtime lookup only on a miss.
- The stub JIT can be entered from any `ip` and will execute the remainder of the frame via runtime stubs. Debug mode must be off.

VM-aware calling convention
- All VM-aware stubs have `extern "C"` ABI and take `*mut VmRuntime` as the first parameter.
- Stubs return:
  - `0` on success.
  - `-1` on error (and store the corresponding `VmError` into an internal `jit_trap` field).
  - For truthiness, `nyx_jit_pop_truthy` returns `0/1` for false/true and `-1` for error.
- `RET`/`HALT` do not directly unwind VM frames. Instead, stubs set an internal `jit_retval` that the caller consumes.

VM-aware safety invariants
- Runtime stubs must never unwind across `extern "C"`. All stubs are wrapped with `catch_unwind` and convert panics into a `RuntimeError` trap.
- Stubs validate operands (negative indices/lengths, out-of-range accesses, type mismatches) and report errors via `VmError` rather than panicking.

VM-aware opcode coverage
  - Control flow: `HALT`, `NOP`, `RET`, `JMP`, `JZ`, `JNZ`, `CALL`, `CallExt`
  - Stack: `PUSH`, `PUSHM`, `POP`, `DUP`, `SWAP`
  - Locals: `LOAD`, `STORE`, `CLOSURE`, `ClosureRef`, `ClosureRefStack`
  - Arithmetic/compare: `ADD`, `SUB`, `MUL`, `DIV`, `NEG`, `EQ`, `NE`, `LT`, `GT`, `LE`, `GE`
  - Heap/object/array/globals: `NewArray`, `NewObj`, `GetField`, `SetField`, `GetIndex`, `SetIndex`, `LEN`, `SLICE`, `ALLOC`, `FREE`, `GetGlobal`, `SetGlobal`, `GetGlobalM`, `SetGlobalM`

--------------------------------------------------------------------------------

## 8. Heap and Garbage Collection
The VM provides a simple heap with optional GC:
- Heap is a vector of slots, each slot is an optional `Box<HeapSlot>`.
- Boxed slots ensure that pointers to `HeapSlot` metadata (generation, version) remain stable and non-moving during execution, which is required for safe JIT IC inlining.
- Each `HeapSlot` is `repr(C)` and stores `generation` (u64), `version` (u64), `value`, and `size_bytes`.
- A free list is used for reuse.
- Heap capacity is enforced by `VmConfig.heap_size`.

Allocation:
- `ALLOC` pops a value and places it into the heap.
- If space is insufficient and GC is enabled, a GC pass runs.
- If still insufficient, the VM errors with `OutOfMemory`.

GC:
- Mark‑and‑sweep.
- Roots are all stack values and global values.
- Marks follow `Pointer`, `Array`, and `Object` references.
- Sweep frees unmarked slots, returns indices to free list.

This GC is deterministic and non‑moving. Pointers remain stable for the duration of the process.

--------------------------------------------------------------------------------

## 9. Execution Limits and Safety
The VM enforces:
- `max_stack_size` to prevent stack overflow.
- `max_call_depth` to prevent unbounded recursion.
- `heap_size` to bound memory usage.
- `max_instructions` to prevent infinite loops and runaway execution.
- `max_exec_time_ms` to enforce wall‑clock limits.

Limits are enforced in the main execution loop. Exceeding a limit returns a VM error rather than panicking or hanging.

Operand validation:
- Negative indices or lengths are rejected.
- Out‑of‑range indices or jump targets are rejected.
- Type mismatches are rejected.

--------------------------------------------------------------------------------

## 10. Native Calls
Native functions can be registered and invoked via `CallExt` or dynamic `CALL`:

Registration:
- `NyxVm::register(name, arity, func)`
- `func` has signature `fn(&[Value]) -> Result<Value, String>`.

Invocation:
- `CallExt name_idx, argc` expects a string constant at `name_idx`.
- It pops `argc` arguments from the stack, checks arity, and calls the host.
- `CALL -1, argc` supports `NativeFunc` values (callee on stack) in addition to `Function` and `Closure`.
- Result is pushed onto the stack.

Inline caches:
- Constant string cache for fast native/global name resolution.
- Global value cache keyed by constant index and invalidated by a global version counter.
- Native function resolution cache.
- Polymorphic call-site cache for `CALL -1`.
- Attribute and index caches for pointer-backed objects/arrays (heap-slot generation + version guarded).

This provides a stable integration point for engine runtime services and system APIs.

--------------------------------------------------------------------------------

## 11. Error Model
Errors are reported through `VmError`:
- `OutOfMemory`, `StackOverflow`, `StackUnderflow`.
- `InvalidOpCode`, `InvalidOperand`.
- `DivisionByZero`.
- `UndefinedVariable`, `UndefinedFunction`.
- `TypeError`, `RuntimeError`, `IoError`.
- `InstructionLimitExceeded`, `TimeLimitExceeded`.
- `Breakpoint` for debug mode.

Errors are propagated as `VmResult<T>` and should be handled by embedding layers.

--------------------------------------------------------------------------------

## 12. Determinism and Portability
The VM is designed to be deterministic, provided:
- The host native functions are deterministic.
- The execution limits are configured consistently.
- Floating‑point behavior is consistent across platforms.

The VM uses standard Rust data structures and does not depend on platform‑specific behavior beyond the host environment.

--------------------------------------------------------------------------------

## 13. Testing and Validation
Tests cover:
- Basic VM initialization and config.
- Arithmetic, call/return, and control‑flow behavior.
- Instruction limit enforcement.
- Native call flow.
- Heap allocation and GC pressure.
- Operand validation for negative indices.
- A loop soak smoke test.

Recommended additional validation:
- Cross‑platform CI testing.
- Fuzzing bytecode parser and runtime.
- Micro‑benchmarks for opcode throughput and GC pressure.

--------------------------------------------------------------------------------

## 14. Embedding Guide
Basic embedding flow:
1. Build or load `BytecodeModule`.
2. Create VM with `NyxVm::new(VmConfig)`.
3. Register native functions if needed.
4. Load modules with `NyxVm::load`.
5. Execute `NyxVm::run` or `NyxVm::run_function`.

Example (pseudo‑flow):
- Create module, add a `main` function.
- Register a native `sum`.
- Run `main`.

The VM owns module storage and global state; create new VM instances for isolated runs.

--------------------------------------------------------------------------------

## 15. Production Readiness & Performance
The Nyx VM is now production-ready with the following hardened features and optimizations:
- **Hardened JIT Runtime**:
  - **Yield Mechanism**: Resolved JIT hangs in async/closure code by implementing a mandatory yield after `CALL` instructions.
  - **IP Synchronization**: Ensures accurate instruction pointer tracking for reliable stack traces and traps.
  - **Security Guards**: Robust enforcement of stack overflow and arity checks in JIT-generated code.
- **Deep Tier 1 Optimizations**:
  - **Extensive Inlining**: Nearly all common computational opcodes (`ADD`, `SUB`, `MUL`, `MOD`, `BAND`, `BOR`, `BXOR`, `BNOT`, `EQ`, `JZ`, `JNZ`) are inlined as fast-path instructions.
  - **Inline Caching (IC)**: Specialized inlined IC checks for `GetField`, `SetField`, `GetIndex`, and `SetIndex`.
- **Hardened Mutation**:
  - **Autoboxing**: Automatic heap allocation for `Value::Object` and `Value::Array` on mutation ensures consistent performance via pointer-based ICs.
- **Scalability**:
  - **High Arity**: Numeric JIT supports high-performance direct calls for functions with up to **32** arguments.

## 16. Future Work
While production-ready, subsequent iterations may include:
- **Register Allocation**: Transitioning to a full SSA-based register allocator to further reduce stack memory traffic in hot loops.
- **SIMD Support**: Specialized JIT lowering for vector operations.

--------------------------------------------------------------------------------

End of document.

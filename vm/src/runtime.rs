//! Nyx VM Runtime
//!
//! This module provides the runtime engine for executing Nyx bytecode.

use std::cell::RefCell;
use std::collections::HashMap;
use std::mem::size_of;
use std::time::Instant;

use crate::bytecode::{BytecodeInstr, BytecodeModule, Function, NativeFunction, OpCode, Value};
use crate::emitter::BytecodeOptimizer;
use crate::jit;
use crate::{VmConfig, VmError, VmResult};

/// Call frame for function execution
#[derive(Debug, Clone)]
pub struct Frame {
    /// Module name for this frame
    pub module_name: String,
    /// Function index for this frame
    pub function_idx: usize,
    /// Function being executed
    pub function: Function,
    /// Instruction pointer
    pub ip: usize,
    /// Stack base for this frame
    pub stack_base: usize,
    /// Number of locals
    pub num_locals: usize,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct HeapSlot {
    pub generation: u64,
    pub version: u64,
    pub value: Value,
    pub size_bytes: usize,
    pub is_upvalue: bool,
}

#[derive(Debug)]
struct Heap {
    slots: Vec<Option<Box<HeapSlot>>>,
    free_list: Vec<usize>,
    used_bytes: usize,
    capacity_bytes: usize,
    next_generation: u64,
    /// Total bytes ever allocated
    total_allocated: u64,
    /// Total bytes ever freed
    total_freed: u64,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct AttributeIC {
    pub target_ptr: usize,
    pub generation: u64,
    pub version: u64,
    pub value: Value,
}

impl Default for AttributeIC {
    fn default() -> Self {
        Self {
            target_ptr: usize::MAX,
            generation: 0,
            version: 0,
            value: Value::Null,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct IndexIC {
    pub target_ptr: usize,
    pub index: i64,
    pub generation: u64,
    pub version: u64,
    pub value: Value,
}

impl Default for IndexIC {
    fn default() -> Self {
        Self {
            target_ptr: usize::MAX,
            index: -1,
            generation: 0,
            version: 0,
            value: Value::Null,
        }
    }
}

impl Heap {
    fn new(capacity_bytes: usize) -> Self {
        Self {
            slots: Vec::new(),
            free_list: Vec::new(),
            used_bytes: 0,
            capacity_bytes,
            next_generation: 1,
            total_allocated: 0,
            total_freed: 0,
        }
    }
}

/// VM runtime state
pub struct VmRuntime {
    /// Call stack
    pub frames: Vec<Frame>,
    /// Operand stack
    pub stack: Vec<Value>,
    /// Global variables
    globals: HashMap<String, Value>,
    /// Modules
    modules: HashMap<String, BytecodeModule>,
    /// Native functions
    natives: HashMap<String, NativeFunction>,
    /// Cached constant strings for inline lookups
    const_string_cache: RefCell<HashMap<(String, usize, usize), String>>,
    /// Cached globals (versioned)
    global_cache: HashMap<(String, usize, usize), (u64, Value)>,
    /// Global version counter
    global_version: u64,
    /// Cached native function resolution
    native_cache: HashMap<String, NativeFunction>,
    /// Dynamic call site cache (polymorphic)
    callsite_cache: HashMap<(String, usize, usize), Vec<CallCacheEntry>>,
    /// Field lookup cache for pointer-backed objects
    field_cache: HashMap<(usize, String), (u64, u64, Value)>,
    /// Index lookup cache for pointer-backed arrays
    index_cache: HashMap<(usize, i64), (u64, u64, Value)>,
    /// Optimized function cache (tier-0 JIT)
    optimized_cache: HashMap<(String, usize), Function>,
    /// Cranelift JIT engine
    pub jit_engine: Option<jit::Engine>,
    /// Configuration
    config: VmConfig,
    /// Heap storage
    heap: Heap,
    /// Instruction count for debugging
    instruction_count: u64,
    /// Execution start time
    start_time: Option<Instant>,
    /// Call depth
    call_depth: usize,
    /// VM-aware JIT trap (set by runtime stubs; consumed by the caller).
    #[cfg(feature = "jit")]
    jit_trap: Option<VmError>,
    /// VM-aware JIT return value (set by the `RET` stub; consumed by the caller).
    #[cfg(feature = "jit")]
    jit_retval: Option<Value>,
}

impl VmRuntime {
    /// Create new VM runtime
    pub fn new(config: VmConfig) -> Self {
        let heap = Heap::new(config.heap_size);
        let jit_engine = if config.enable_jit {
            jit::new_engine().ok()
        } else {
            None
        };
        Self {
            frames: Vec::new(),
            stack: Vec::with_capacity(config.max_stack_size),
            globals: HashMap::new(),
            modules: HashMap::new(),
            natives: HashMap::new(),
            const_string_cache: RefCell::new(HashMap::new()),
            optimized_cache: HashMap::new(),
            global_cache: HashMap::new(),
            global_version: 0,
            native_cache: HashMap::new(),
            callsite_cache: HashMap::new(),
            field_cache: HashMap::new(),
            index_cache: HashMap::new(),
            jit_engine,
            config,
            heap,
            instruction_count: 0,
            start_time: None,
            call_depth: 0,
            #[cfg(feature = "jit")]
            jit_trap: None,
            #[cfg(feature = "jit")]
            jit_retval: None,
        }
    }

    #[cfg(feature = "jit")]
    fn jit_clear_state(&mut self) {
        self.jit_trap = None;
        self.jit_retval = None;
    }

    #[cfg(feature = "jit")]
    fn jit_set_trap(&mut self, err: VmError) {
        if self.jit_trap.is_none() {
            self.jit_trap = Some(err);
        }
    }

    #[cfg(feature = "jit")]
    fn jit_take_trap(&mut self) -> Option<VmError> {
        self.jit_trap.take()
    }

    #[cfg(feature = "jit")]
    fn jit_set_retval(&mut self, value: Value) {
        self.jit_retval = Some(value);
    }

    #[cfg(feature = "jit")]
    fn jit_take_retval(&mut self) -> Option<Value> {
        self.jit_retval.take()
    }

    fn push_value(&mut self, value: Value) -> VmResult<()> {
        if self.stack.len() >= self.config.max_stack_size {
            return Err(VmError::StackOverflow);
        }
        self.stack.push(value);
        Ok(())
    }

    fn pop_value(&mut self) -> VmResult<Value> {
        self.stack.pop().ok_or(VmError::StackUnderflow)
    }

    fn operand_i32(&self, instr: &BytecodeInstr, idx: usize, name: &str) -> VmResult<i32> {
        instr
            .operand(idx)
            .ok_or_else(|| VmError::InvalidOperand(format!("missing operand {}", name)))
    }

    fn operand_usize(&self, instr: &BytecodeInstr, idx: usize, name: &str) -> VmResult<usize> {
        let raw = self.operand_i32(instr, idx, name)?;
        if raw < 0 {
            return Err(VmError::InvalidOperand(format!(
                "negative operand {}",
                name
            )));
        }
        Ok(raw as usize)
    }

    fn resolve_constant(&self, frame: &Frame, idx: usize) -> VmResult<Value> {
        if let Some(value) = frame.function.constants.get(idx) {
            return Ok(value.clone());
        }
        let module = self.modules.get(&frame.module_name).ok_or_else(|| {
            VmError::RuntimeError(format!("Module not found: {}", frame.module_name))
        })?;
        let offset = idx.saturating_sub(frame.function.constants.len());
        module
            .constants
            .get(offset)
            .cloned()
            .ok_or_else(|| VmError::InvalidOperand(format!("constant index {}", idx)))
    }

    fn resolve_constant_string(&self, frame: &Frame, idx: usize, label: &str) -> VmResult<String> {
        if let Some(value) = self.const_string_cache.borrow().get(&(
            frame.module_name.clone(),
            frame.function_idx,
            idx,
        )) {
            return Ok(value.clone());
        }
        match self.resolve_constant(frame, idx)? {
            Value::String(s) => {
                self.const_string_cache.borrow_mut().insert(
                    (frame.module_name.clone(), frame.function_idx, idx),
                    s.clone(),
                );
                Ok(s)
            }
            _ => Err(VmError::InvalidOperand(format!("{} must be string", label))),
        }
    }

    fn resolve_module_constant_string(
        &self,
        frame: &Frame,
        idx: usize,
        label: &str,
    ) -> VmResult<String> {
        let module = self.modules.get(&frame.module_name).ok_or_else(|| {
            VmError::RuntimeError(format!("Module not found: {}", frame.module_name))
        })?;
        match module.constants.get(idx) {
            Some(Value::String(s)) => Ok(s.clone()),
            Some(_) => Err(VmError::InvalidOperand(format!("{} must be string", label))),
            None => Err(VmError::InvalidOperand(format!(
                "module constant index {}",
                idx
            ))),
        }
    }

    fn resolve_native(&mut self, name: &str) -> VmResult<NativeFunction> {
        if let Some(native) = self.native_cache.get(name) {
            return Ok(native.clone());
        }
        let native = self
            .natives
            .get(name)
            .cloned()
            .ok_or_else(|| VmError::UndefinedFunction(name.to_string()))?;
        self.native_cache.insert(name.to_string(), native.clone());
        Ok(native)
    }

    fn get_function_for_exec(&mut self, module_name: &str, func_idx: usize) -> VmResult<Function> {
        if self.config.enable_jit {
            if let Some(func) = self
                .optimized_cache
                .get(&(module_name.to_string(), func_idx))
            {
                return Ok(func.clone());
            }
        }
        let module = self
            .modules
            .get(module_name)
            .ok_or_else(|| VmError::RuntimeError(format!("Module not found: {}", module_name)))?;
        let mut func = module
            .get_function(func_idx)
            .ok_or_else(|| VmError::UndefinedFunction(format!("Function index: {}", func_idx)))?
            .clone();
        if self.config.enable_jit {
            BytecodeOptimizer::optimize(&mut func);
            self.optimized_cache
                .insert((module_name.to_string(), func_idx), func.clone());
        }
        Ok(func)
    }

    fn check_limits(&self) -> VmResult<()> {
        if self.config.max_instructions > 0 && self.instruction_count > self.config.max_instructions
        {
            return Err(VmError::InstructionLimitExceeded(
                self.config.max_instructions,
            ));
        }
        if let Some(start) = self.start_time {
            if self.config.max_exec_time_ms > 0 {
                let elapsed_ms = start.elapsed().as_millis() as u64;
                if elapsed_ms > self.config.max_exec_time_ms {
                    return Err(VmError::TimeLimitExceeded(self.config.max_exec_time_ms));
                }
            }
        }
        Ok(())
    }

    fn heap_alloc(&mut self, value: Value, size_hint: usize) -> VmResult<usize> {
        let size_bytes = if size_hint == 0 {
            size_of::<Value>()
        } else {
            size_hint
        };
        if self.heap.used_bytes.saturating_add(size_bytes) > self.heap.capacity_bytes
            && self.config.enable_gc
        {
            self.collect_garbage();
        }
        if self.heap.used_bytes.saturating_add(size_bytes) > self.heap.capacity_bytes {
            return Err(VmError::OutOfMemory);
        }
        let generation = self.heap.next_generation;
        self.heap.next_generation = self.heap.next_generation.saturating_add(1);
        let slot = HeapSlot {
            generation,
            version: 0,
            value,
            size_bytes,
            is_upvalue: false,
        };
        if let Some(idx) = self.heap.free_list.pop() {
            if idx < self.heap.slots.len() {
                self.heap.slots[idx] = Some(Box::new(slot));
                self.heap.used_bytes += size_bytes;
                self.heap.total_allocated += size_bytes as u64;
                return Ok(idx);
            }
        }
        let idx = self.heap.slots.len();
        self.heap.slots.push(Some(Box::new(slot)));
        self.heap.total_allocated += size_bytes as u64;
        self.heap.used_bytes += size_bytes;
        Ok(idx)
    }

    fn heap_alloc_upvalue(&mut self, value: Value) -> VmResult<usize> {
        let size_bytes = size_of::<Value>();
        if self.heap.used_bytes.saturating_add(size_bytes) > self.heap.capacity_bytes
            && self.config.enable_gc
        {
            self.collect_garbage();
        }
        if self.heap.used_bytes.saturating_add(size_bytes) > self.heap.capacity_bytes {
            return Err(VmError::OutOfMemory);
        }
        let generation = self.heap.next_generation;
        self.heap.next_generation = self.heap.next_generation.saturating_add(1);
        let slot = HeapSlot {
            generation,
            version: 0,
            value,
            size_bytes,
            is_upvalue: true,
        };
        if let Some(idx) = self.heap.free_list.pop() {
            if idx < self.heap.slots.len() {
                self.heap.slots[idx] = Some(Box::new(slot));
                self.heap.used_bytes += size_bytes;
                self.heap.total_allocated += size_bytes as u64;
                return Ok(idx);
            }
        }
        let idx = self.heap.slots.len();
        self.heap.slots.push(Some(Box::new(slot)));
        self.heap.total_allocated += size_bytes as u64;
        self.heap.used_bytes += size_bytes;
        Ok(idx)
    }

    fn ensure_upvalue_cell(&mut self, frame: &Frame, local_idx: usize) -> VmResult<usize> {
        let pos = frame.stack_base + local_idx;
        if pos >= self.stack.len() {
            return Err(VmError::InvalidOperand(format!(
                "local index {}",
                local_idx
            )));
        }
        if let Value::Pointer(ptr) = &self.stack[pos] {
            let slot = self.heap.slots.get(*ptr).and_then(|s| s.as_ref());
            if let Some(slot) = slot {
                if slot.is_upvalue {
                    return Ok(*ptr);
                }
            }
        }
        let value = self.stack[pos].clone();
        let ptr = self.heap_alloc_upvalue(value)?;
        self.stack[pos] = Value::Pointer(ptr);
        Ok(ptr)
    }

    fn heap_free(&mut self, ptr: usize) -> VmResult<()> {
        if ptr >= self.heap.slots.len() {
            return Err(VmError::InvalidOperand(format!("invalid pointer {}", ptr)));
        }
        if let Some(slot) = self.heap.slots[ptr].take() {
            self.heap.used_bytes = self.heap.used_bytes.saturating_sub(slot.size_bytes);
            self.heap.total_freed += slot.size_bytes as u64;
            self.heap.free_list.push(ptr);
            Ok(())
        } else {
            Err(VmError::InvalidOperand(format!("double free {}", ptr)))
        }
    }

    fn heap_get(&self, ptr: usize) -> VmResult<&Value> {
        self.heap
            .slots
            .get(ptr)
            .and_then(|s| s.as_ref())
            .map(|s| &s.value)
            .ok_or_else(|| VmError::InvalidOperand(format!("invalid pointer {}", ptr)))
    }

    fn heap_get_mut(&mut self, ptr: usize) -> VmResult<&mut Value> {
        self.heap
            .slots
            .get_mut(ptr)
            .and_then(|s| s.as_mut())
            .map(|s| &mut s.value)
            .ok_or_else(|| VmError::InvalidOperand(format!("invalid pointer {}", ptr)))
    }

    #[allow(dead_code)]
    pub(crate) fn heap_get_field(&self, ptr: usize, field: &str) -> VmResult<Value> {
        match self.heap_get(ptr)? {
            Value::Object(obj) => obj
                .get(field)
                .cloned()
                .ok_or_else(|| VmError::UndefinedVariable(field.to_string())),
            _ => Err(VmError::TypeError("GetField expects object".to_string())),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn heap_set_field(&mut self, ptr: usize, field: &str, value: Value) -> VmResult<()> {
        match self.heap_get_mut(ptr)? {
            Value::Object(obj) => {
                obj.insert(field.to_string(), value);
                Ok(())
            }
            _ => Err(VmError::TypeError("SetField expects object".to_string())),
        }
    }

    fn call_dynamic(
        &mut self,
        callee: Value,
        num_args: usize,
        module_name: &str,
        callsite: (String, usize, usize),
    ) -> VmResult<ControlFlow> {
        if let Some(entries) = self.callsite_cache.get(&callsite) {
            if let Some(hit) = self.match_call_cache(entries, &callee) {
                return self.call_dynamic_with_cache(hit, callee, num_args, module_name);
            }
        }
        match callee {
            Value::Function(func_idx) => {
                let result = self.call_function_index(func_idx, num_args, module_name);
                self.update_callsite_cache(callsite, CallCacheEntry::Function(func_idx));
                result
            }
            Value::Closure(closure) => {
                let func_idx = closure.function_idx;
                let up_len = closure.upvalues.len();
                let result = self.call_closure(closure, num_args, module_name);
                self.update_callsite_cache(callsite, CallCacheEntry::Closure(func_idx, up_len));
                result
            }
            Value::NativeFunc(native) => {
                if native.arity != num_args {
                    return Err(VmError::InvalidOperand(format!(
                        "arity mismatch: expected {}, got {}",
                        native.arity, num_args
                    )));
                }
                let mut args = Vec::with_capacity(num_args);
                for _ in 0..num_args {
                    args.push(self.pop_value()?);
                }
                args.reverse();
                let result = (native.func)(&args).map_err(VmError::RuntimeError)?;
                self.push_value(result)?;
                self.update_callsite_cache(callsite, CallCacheEntry::Native(native.name));
                Ok(ControlFlow::Continue)
            }
            _ => Err(VmError::TypeError(
                "CALL expects function/closure/native".to_string(),
            )),
        }
    }

    fn match_call_cache(
        &self,
        entries: &[CallCacheEntry],
        callee: &Value,
    ) -> Option<CallCacheEntry> {
        for entry in entries {
            match (entry, callee) {
                (CallCacheEntry::Function(idx), Value::Function(ci)) if idx == ci => {
                    return Some(entry.clone());
                }
                (CallCacheEntry::Native(name), Value::NativeFunc(n)) if name == &n.name => {
                    return Some(entry.clone());
                }
                (CallCacheEntry::Closure(idx, len), Value::Closure(c))
                    if *idx == c.function_idx && *len == c.upvalues.len() =>
                {
                    return Some(entry.clone());
                }
                _ => {}
            }
        }
        None
    }

    fn call_dynamic_with_cache(
        &mut self,
        entry: CallCacheEntry,
        callee: Value,
        num_args: usize,
        module_name: &str,
    ) -> VmResult<ControlFlow> {
        match entry {
            CallCacheEntry::Function(func_idx) => {
                self.call_function_index(func_idx, num_args, module_name)
            }
            CallCacheEntry::Native(name) => {
                let native = self.resolve_native(&name)?;
                if native.arity != num_args {
                    return Err(VmError::InvalidOperand(format!(
                        "arity mismatch: expected {}, got {}",
                        native.arity, num_args
                    )));
                }
                let mut args = Vec::with_capacity(num_args);
                for _ in 0..num_args {
                    args.push(self.pop_value()?);
                }
                args.reverse();
                let result = (native.func)(&args).map_err(VmError::RuntimeError)?;
                self.push_value(result)?;
                Ok(ControlFlow::Continue)
            }
            CallCacheEntry::Closure(func_idx, up_len) => match callee {
                Value::Closure(c) if c.function_idx == func_idx && c.upvalues.len() == up_len => {
                    self.call_closure(c, num_args, module_name)
                }
                _ => Err(VmError::TypeError(
                    "CALL expects function/closure/native".to_string(),
                )),
            },
        }
    }

    fn update_callsite_cache(&mut self, callsite: (String, usize, usize), entry: CallCacheEntry) {
        let entries = self.callsite_cache.entry(callsite).or_default();
        if entries.iter().any(|e| match (e, &entry) {
            (CallCacheEntry::Function(a), CallCacheEntry::Function(b)) => a == b,
            (CallCacheEntry::Native(a), CallCacheEntry::Native(b)) => a == b,
            (CallCacheEntry::Closure(a_idx, a_len), CallCacheEntry::Closure(b_idx, b_len)) => {
                a_idx == b_idx && a_len == b_len
            }
            _ => false,
        }) {
            return;
        }
        if entries.len() >= 4 {
            entries.remove(0);
        }
        entries.push(entry);
    }

    fn call_function_index(
        &mut self,
        func_idx: usize,
        num_args: usize,
        module_name: &str,
    ) -> VmResult<ControlFlow> {
        let func = self.get_function_for_exec(module_name, func_idx)?;
        if func.arity != num_args {
            return Err(VmError::InvalidOperand(format!(
                "arity mismatch: expected {}, got {}",
                func.arity, num_args
            )));
        }

        if self.config.enable_jit {
            if let Some(engine) = self.jit_engine.as_mut() {
                if let Some(plan) = jit::plan(&func) {
                    let start = self.stack.len().saturating_sub(num_args);
                    let args_slice = self.stack.get(start..).ok_or(VmError::StackUnderflow)?;

                    self.call_depth += 1;
                    if self.call_depth > self.config.max_call_depth {
                        self.call_depth = self.call_depth.saturating_sub(1);
                        return Err(VmError::StackOverflow);
                    }

                    let jit_func = jit::compile(engine, module_name, func_idx, &func, plan);
                    if let Ok(jit_func) = jit_func {
                        match plan.num_kind {
                            jit::JitNumKind::I64 => {
                                let mut int_args = Vec::with_capacity(num_args);
                                for value in args_slice {
                                    match value {
                                        Value::Int(i) => int_args.push(*i),
                                        _ => {
                                            int_args.clear();
                                            break;
                                        }
                                    }
                                }
                                if int_args.len() == num_args {
                                    self.stack.truncate(start);
                                    let result = jit::call_i64(&jit_func, &int_args);
                                    self.call_depth = self.call_depth.saturating_sub(1);
                                    match plan.ret_kind {
                                        jit::JitRetKind::I64 => {
                                            self.push_value(Value::Int(result))?
                                        }
                                        jit::JitRetKind::Bool => {
                                            self.push_value(Value::Bool(result != 0))?
                                        }
                                        jit::JitRetKind::F64 => {}
                                    }
                                    return Ok(ControlFlow::Continue);
                                }
                            }
                            jit::JitNumKind::F64 => {
                                let mut float_args = Vec::with_capacity(num_args);
                                for value in args_slice {
                                    match value {
                                        Value::Float(f) => float_args.push(*f),
                                        _ => {
                                            float_args.clear();
                                            break;
                                        }
                                    }
                                }
                                if float_args.len() == num_args {
                                    self.stack.truncate(start);
                                    self.call_depth = self.call_depth.saturating_sub(1);
                                    match plan.ret_kind {
                                        jit::JitRetKind::F64 => {
                                            let result = jit::call_f64(&jit_func, &float_args);
                                            self.push_value(Value::Float(result))?;
                                        }
                                        jit::JitRetKind::Bool => {
                                            let result =
                                                jit::call_bool_from_f64(&jit_func, &float_args);
                                            self.push_value(Value::Bool(result != 0))?;
                                        }
                                        jit::JitRetKind::I64 => {}
                                    }
                                    return Ok(ControlFlow::Continue);
                                }
                            }
                        }
                    }
                    self.call_depth = self.call_depth.saturating_sub(1);
                }
            }
        }
        self.call_depth += 1;
        if self.call_depth > self.config.max_call_depth {
            return Err(VmError::StackOverflow);
        }
        let frame = Frame {
            module_name: module_name.to_string(),
            function_idx: func_idx,
            function: func.clone(),
            ip: 0,
            stack_base: self.stack.len() - num_args,
            num_locals: func.num_locals,
        };
        let required = frame.stack_base + frame.num_locals;
        while self.stack.len() < required {
            self.push_value(Value::Null)?;
        }
        self.frames.push(frame);
        Ok(ControlFlow::Continue)
    }

    fn call_closure(
        &mut self,
        closure: crate::bytecode::Closure,
        num_args: usize,
        module_name: &str,
    ) -> VmResult<ControlFlow> {
        let func = self.get_function_for_exec(module_name, closure.function_idx)?;
        if func.arity != num_args {
            return Err(VmError::InvalidOperand(format!(
                "arity mismatch: expected {}, got {}",
                func.arity, num_args
            )));
        }
        if func.upvalues.len() != closure.upvalues.len() {
            return Err(VmError::InvalidOperand(format!(
                "upvalue mismatch: expected {}, got {}",
                func.upvalues.len(),
                closure.upvalues.len()
            )));
        }
        if func.num_locals < num_args + closure.upvalues.len() {
            return Err(VmError::InvalidOperand(
                "insufficient locals for closure upvalues".to_string(),
            ));
        }
        self.call_depth += 1;
        if self.call_depth > self.config.max_call_depth {
            return Err(VmError::StackOverflow);
        }
        let frame = Frame {
            module_name: module_name.to_string(),
            function_idx: closure.function_idx,
            function: func.clone(),
            ip: 0,
            stack_base: self.stack.len() - num_args,
            num_locals: func.num_locals,
        };
        let required = frame.stack_base + frame.num_locals;
        while self.stack.len() < required {
            self.push_value(Value::Null)?;
        }
        if !closure.upvalues.is_empty() {
            let start = frame.stack_base + frame.num_locals - closure.upvalues.len();
            for (offset, value) in closure.upvalues.iter().cloned().enumerate() {
                self.stack[start + offset] = value;
            }
        }
        self.frames.push(frame);
        Ok(ControlFlow::Continue)
    }

    fn collect_garbage(&mut self) {
        if self.heap.slots.is_empty() {
            return;
        }
        let mut marks = vec![false; self.heap.slots.len()];
        let mut roots: Vec<Value> = Vec::with_capacity(self.stack.len() + self.globals.len());
        roots.extend(self.stack.iter().cloned());
        roots.extend(self.globals.values().cloned());

        // Scan field and index caches
        for (_, _, val) in self.field_cache.values() {
            roots.push(val.clone());
        }
        for (_, _, val) in self.index_cache.values() {
            roots.push(val.clone());
        }

        // Scan global cache
        for (_, val) in self.global_cache.values() {
            roots.push(val.clone());
        }

        // Scan JIT IC buffers
        #[cfg(feature = "jit")]
        if let Some(engine) = &self.jit_engine {
            for vmjit in engine.vm_functions.values() {
                if let Some(attr_ics) = &vmjit.attr_ic_buffer {
                    for ic in attr_ics.iter() {
                        roots.push(ic.value.clone());
                    }
                }
                if let Some(index_ics) = &vmjit.index_ic_buffer {
                    for ic in index_ics.iter() {
                        roots.push(ic.value.clone());
                    }
                }
            }
        }

        for value in &roots {
            Self::mark_value(value, &self.heap.slots, &mut marks);
        }
        for (idx, slot) in self.heap.slots.iter_mut().enumerate() {
            if slot.is_some() && !marks[idx] {
                if let Some(slot_value) = slot.take() {
                    self.heap.used_bytes =
                        self.heap.used_bytes.saturating_sub(slot_value.size_bytes);
                    self.heap.free_list.push(idx);
                }
            }
        }
    }

    fn mark_value(value: &Value, slots: &Vec<Option<Box<HeapSlot>>>, marks: &mut [bool]) {
        match value {
            Value::Pointer(ptr) => {
                let idx = *ptr;
                if idx >= marks.len() || marks[idx] {
                    return;
                }
                marks[idx] = true;
                let inner = match slots.get(idx).and_then(|s| s.as_ref()) {
                    Some(slot) => slot.value.clone(),
                    None => return,
                };
                Self::mark_value(&inner, slots, marks);
            }
            Value::Closure(c) => {
                for up in &c.upvalues {
                    Self::mark_value(up, slots, marks);
                }
            }
            Value::Array(items) => {
                for item in items {
                    Self::mark_value(item, slots, marks);
                }
            }
            Value::Object(map) => {
                for item in map.values() {
                    Self::mark_value(item, slots, marks);
                }
            }
            _ => {}
        }
    }

    /// Load a module
    pub fn load_module(&mut self, module: BytecodeModule) {
        self.modules.insert(module.name.clone(), module);
    }

    /// Register a native function
    pub fn register_native(
        &mut self,
        name: &str,
        arity: usize,
        func: fn(&[Value]) -> Result<Value, String>,
    ) {
        self.natives.insert(
            name.to_string(),
            NativeFunction {
                name: name.to_string(),
                arity,
                func,
            },
        );
    }

    /// Run the VM
    pub fn run(&mut self, module_name: &str) -> VmResult<Value> {
        let module = self
            .modules
            .get(module_name)
            .ok_or_else(|| VmError::RuntimeError(format!("Module not found: {}", module_name)))?;

        // Find main function
        let main_func_idx = module
            .functions
            .iter()
            .position(|f| f.name == "main")
            .ok_or_else(|| VmError::UndefinedFunction("main".to_string()))?;

        self.run_function(module_name, main_func_idx, vec![])
    }

    /// Run a specific function
    pub fn run_function(
        &mut self,
        module_name: &str,
        func_idx: usize,
        args: Vec<Value>,
    ) -> VmResult<Value> {
        let func = self.get_function_for_exec(module_name, func_idx)?;

        // Reset execution state for this run
        self.instruction_count = 0;
        self.start_time = if self.config.max_exec_time_ms > 0 {
            Some(Instant::now())
        } else {
            None
        };

        // Check call depth
        self.call_depth = 1;
        if self.call_depth > self.config.max_call_depth {
            return Err(VmError::StackOverflow);
        }

        // Check arity
        if func.arity != args.len() {
            return Err(VmError::TypeError(format!(
                "arity mismatch: expected {}, got {}",
                func.arity,
                args.len()
            )));
        }

        // Create new frame
        let frame = Frame {
            module_name: module_name.to_string(),
            function_idx: func_idx,
            function: func.clone(),
            ip: 0,
            stack_base: self.stack.len(),
            num_locals: func.num_locals,
        };

        // Push arguments onto stack
        for arg in args {
            self.push_value(arg)?;
        }
        let required = frame.stack_base + frame.num_locals;
        while self.stack.len() < required {
            self.push_value(Value::Null)?;
        }

        self.frames.push(frame);

        // Execute
        let result = self.execute();

        self.frames.clear();
        self.call_depth = 0;
        self.stack.clear();
        self.start_time = None;

        result
    }

    /// Main execution loop
    fn execute(&mut self) -> VmResult<Value> {
        loop {
            // Snapshot current frame metadata (avoid holding a mutable borrow across optional JIT).
            let (ip, instr_len) = {
                let frame = self
                    .frames
                    .last()
                    .ok_or_else(|| VmError::RuntimeError("No frames".to_string()))?;
                (frame.ip, frame.function.instructions.len())
            };

            // Check IP bounds
            if ip >= instr_len {
                return Ok(Value::Unit);
            }

            // VM-aware JIT: execute the entire frame via runtime stubs when possible.
            #[cfg(feature = "jit")]
            if self.config.enable_jit && !self.config.debug {
                if let Some(engine) = self.jit_engine.as_mut() {
                    let (module_name, function_idx) = {
                        let frame = self
                            .frames
                            .last()
                            .ok_or_else(|| VmError::RuntimeError("No frames".to_string()))?;
                        (frame.module_name.clone(), frame.function_idx)
                    };
                    let func = {
                        let frame = self
                            .frames
                            .last()
                            .ok_or_else(|| VmError::RuntimeError("No frames".to_string()))?;
                        frame.function.clone()
                    };
                    if jit::vm_plan(&func) {
                        if let Ok(jit_func) =
                            jit::compile_vm(engine, &module_name, function_idx, &func)
                        {
                            self.jit_clear_state();
                            // SAFETY: `self` is a valid, non-null, properly initialised
                            // `VmRuntime` pointer and is not aliased during this call.
                            let status = unsafe {
                                jit::call_vm(&jit_func, self as *mut VmRuntime, ip as i32)
                            };
                            if status == 0 {
                                let val = self.jit_take_retval().unwrap_or(Value::Unit);
                                let finished = self.frames.pop().ok_or_else(|| {
                                    VmError::RuntimeError("No frames".to_string())
                                })?;
                                self.call_depth = self.call_depth.saturating_sub(1);
                                self.stack.truncate(finished.stack_base);
                                if self.frames.is_empty() {
                                    return Ok(val);
                                }
                                self.push_value(val)?;
                                continue;
                            }
                            if status == 1 {
                                // JIT yielded (e.g. after a CALL).
                                // Just continue the VM loop, which will execute the new frame.
                                continue;
                            }
                            if let Some(err) = self.jit_take_trap() {
                                return Err(err);
                            }
                            return Err(VmError::RuntimeError("VM-aware JIT failed".to_string()));
                        }
                    }
                }
            }

            // Get current frame (mutable) for interpreter execution.
            let frame = self
                .frames
                .last_mut()
                .ok_or_else(|| VmError::RuntimeError("No frames".to_string()))?;

            // Get instruction
            let instr_idx = frame.ip;
            let instr = frame.function.instructions[instr_idx].clone();
            frame.ip = frame.ip.saturating_add(1);
            self.instruction_count = self.instruction_count.saturating_add(1);
            self.check_limits()?;

            // Call optional step hook
            if let Some(mut hook) = self.config.on_step.take() {
                // We must construct a temporary NyxVm to pass to the hook
                // which is safe because we just pass it a pointer reference effectively.
                // However, the hook signature takes NyxVm. To avoid lifetime/borrow
                // checker issues, the caller will need to access state carefully.
                // Given the design, let's just use a pointer to avoid borrowing issues in the hook.
                let ptr = self as *mut VmRuntime;
                let vm_ref = unsafe { &mut *(ptr as *mut crate::NyxVm) };

                let res = hook(vm_ref, &instr, instr_idx);
                self.config.on_step = Some(hook);
                res?
            }

            // Execute instruction
            match self.execute_instruction(instr, instr_idx)? {
                ControlFlow::Continue => continue,
                ControlFlow::Return(val) => {
                    let finished = self
                        .frames
                        .pop()
                        .ok_or_else(|| VmError::RuntimeError("No frames".to_string()))?;
                    self.call_depth = self.call_depth.saturating_sub(1);
                    self.stack.truncate(finished.stack_base);
                    if self.frames.is_empty() {
                        return Ok(val);
                    }
                    self.push_value(val)?;
                }
                ControlFlow::Breakpoint(line) => {
                    if self.config.debug {
                        return Err(VmError::Breakpoint(line));
                    }
                }
            }
        }
    }

    /// Execute a single instruction
    fn execute_instruction(
        &mut self,
        instr: BytecodeInstr,
        instr_idx: usize,
    ) -> VmResult<ControlFlow> {
        match instr.opcode {
            OpCode::HALT => Ok(ControlFlow::Return(Value::Unit)),

            OpCode::NOP => Ok(ControlFlow::Continue),

            OpCode::PUSH => {
                let idx = self.operand_usize(&instr, 0, "constant index")?;
                let frame = self
                    .frames
                    .last()
                    .ok_or_else(|| VmError::RuntimeError("No frames".to_string()))?;
                let value = self.resolve_constant(frame, idx)?;
                self.push_value(value)?;
                Ok(ControlFlow::Continue)
            }

            OpCode::POP => {
                let _ = self.pop_value()?;
                Ok(ControlFlow::Continue)
            }

            OpCode::DUP => {
                let top = self.stack.last().ok_or(VmError::StackUnderflow)?.clone();
                self.push_value(top)?;
                Ok(ControlFlow::Continue)
            }

            OpCode::DUP2 => {
                if self.stack.len() < 2 {
                    return Err(VmError::StackUnderflow);
                }
                let len = self.stack.len();
                let a = self.stack[len - 2].clone();
                let b = self.stack[len - 1].clone();
                self.push_value(a)?;
                self.push_value(b)?;
                Ok(ControlFlow::Continue)
            }

            OpCode::SWAP => {
                if self.stack.len() < 2 {
                    return Err(VmError::StackUnderflow);
                }
                let len = self.stack.len();
                self.stack.swap(len - 1, len - 2);
                Ok(ControlFlow::Continue)
            }

            OpCode::ROT => {
                if self.stack.len() < 3 {
                    return Err(VmError::StackUnderflow);
                }
                let len = self.stack.len();
                let a = self.stack.remove(len - 3);
                self.stack.push(a);
                Ok(ControlFlow::Continue)
            }

            OpCode::PICK => {
                let idx = self.operand_usize(&instr, 0, "pick index")?;
                if idx >= self.stack.len() {
                    return Err(VmError::InvalidOperand(format!("pick index {}", idx)));
                }
                let pos = self.stack.len() - 1 - idx;
                let value = self.stack[pos].clone();
                self.push_value(value)?;
                Ok(ControlFlow::Continue)
            }

            OpCode::PUT => {
                let idx = self.operand_usize(&instr, 0, "put index")?;
                let value = self.pop_value()?;
                if idx >= self.stack.len() {
                    return Err(VmError::InvalidOperand(format!("put index {}", idx)));
                }
                let pos = self.stack.len() - 1 - idx;
                self.stack[pos] = value;
                Ok(ControlFlow::Continue)
            }

            OpCode::ADD => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                match (&a, &b) {
                    (Value::String(s1), Value::String(s2)) => {
                        let result = Value::String(format!("{}{}", s1, s2));
                        self.push_value(result)?;
                    }
                    _ => {
                        let result = self.binary_op(&a, &b, |a, b| a + b)?;
                        self.push_value(result)?;
                    }
                }
                Ok(ControlFlow::Continue)
            }

            OpCode::SUB => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                let result = self.binary_op(&a, &b, |a, b| a - b)?;
                self.push_value(result)?;
                Ok(ControlFlow::Continue)
            }

            OpCode::MUL => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                let result = self.binary_op(&a, &b, |a, b| a * b)?;
                self.push_value(result)?;
                Ok(ControlFlow::Continue)
            }

            OpCode::DIV => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                if self.is_zero(&b) {
                    return Err(VmError::DivisionByZero);
                }
                let result = self.binary_op(&a, &b, |a, b| a / b)?;
                self.push_value(result)?;
                Ok(ControlFlow::Continue)
            }

            OpCode::MOD => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                if self.is_zero(&b) {
                    return Err(VmError::DivisionByZero);
                }
                let result = self.binary_op(&a, &b, |a, b| a % b)?;
                self.push_value(result)?;
                Ok(ControlFlow::Continue)
            }

            OpCode::INC => {
                let a = self.pop_value()?;
                let result = self.unary_op(&a, |a| a + 1.0)?;
                self.push_value(result)?;
                Ok(ControlFlow::Continue)
            }

            OpCode::DEC => {
                let a = self.pop_value()?;
                let result = self.unary_op(&a, |a| a - 1.0)?;
                self.push_value(result)?;
                Ok(ControlFlow::Continue)
            }

            OpCode::POW => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                let result = self.binary_op(&a, &b, |a, b| a.powf(b))?;
                self.push_value(result)?;
                Ok(ControlFlow::Continue)
            }

            OpCode::DivRem => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                match (a, b) {
                    (Value::Int(ai), Value::Int(bi)) => {
                        if bi == 0 {
                            return Err(VmError::DivisionByZero);
                        }
                        let div = ai / bi;
                        let rem = ai % bi;
                        self.push_value(Value::Int(div))?;
                        self.push_value(Value::Int(rem))?;
                        Ok(ControlFlow::Continue)
                    }
                    _ => Err(VmError::TypeError(
                        "DivRem expects int operands".to_string(),
                    )),
                }
            }

            OpCode::ABS => {
                let a = self.pop_value()?;
                let result = match a {
                    Value::Int(i) => Value::Int(i.abs()),
                    Value::Float(f) => Value::Float(f.abs()),
                    _ => {
                        return Err(VmError::TypeError(
                            "ABS expects numeric operand".to_string(),
                        ))
                    }
                };
                self.push_value(result)?;
                Ok(ControlFlow::Continue)
            }

            OpCode::MIN => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                let result = match (a, b) {
                    (Value::Int(ai), Value::Int(bi)) => Value::Int(ai.min(bi)),
                    (Value::Float(af), Value::Float(bf)) => Value::Float(af.min(bf)),
                    (Value::Int(ai), Value::Float(bf)) => Value::Float((ai as f64).min(bf)),
                    (Value::Float(af), Value::Int(bi)) => Value::Float(af.min(bi as f64)),
                    _ => {
                        return Err(VmError::TypeError(
                            "MIN expects numeric operands".to_string(),
                        ))
                    }
                };
                self.push_value(result)?;
                Ok(ControlFlow::Continue)
            }

            OpCode::MAX => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                let result = match (a, b) {
                    (Value::Int(ai), Value::Int(bi)) => Value::Int(ai.max(bi)),
                    (Value::Float(af), Value::Float(bf)) => Value::Float(af.max(bf)),
                    (Value::Int(ai), Value::Float(bf)) => Value::Float((ai as f64).max(bf)),
                    (Value::Float(af), Value::Int(bi)) => Value::Float(af.max(bi as f64)),
                    _ => {
                        return Err(VmError::TypeError(
                            "MAX expects numeric operands".to_string(),
                        ))
                    }
                };
                self.push_value(result)?;
                Ok(ControlFlow::Continue)
            }

            OpCode::NEG => {
                let a = self.pop_value()?;
                let result = self.unary_op(&a, |a| -a)?;
                self.push_value(result)?;
                Ok(ControlFlow::Continue)
            }

            OpCode::NOT => {
                let a = self.pop_value()?;
                self.push_value(Value::Bool(!a.is_truthy()))?;
                Ok(ControlFlow::Continue)
            }

            OpCode::EQ => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                self.push_value(Value::Bool(a == b))?;
                Ok(ControlFlow::Continue)
            }

            OpCode::NE => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                self.push_value(Value::Bool(a != b))?;
                Ok(ControlFlow::Continue)
            }

            OpCode::LT => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                let result = self.compare_op(&a, &b, |a, b| a < b)?;
                self.push_value(Value::Bool(result))?;
                Ok(ControlFlow::Continue)
            }

            OpCode::GT => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                let result = self.compare_op(&a, &b, |a, b| a > b)?;
                self.push_value(Value::Bool(result))?;
                Ok(ControlFlow::Continue)
            }

            OpCode::LE => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                let result = self.compare_op(&a, &b, |a, b| a <= b)?;
                self.push_value(Value::Bool(result))?;
                Ok(ControlFlow::Continue)
            }

            OpCode::GE => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                let result = self.compare_op(&a, &b, |a, b| a >= b)?;
                self.push_value(Value::Bool(result))?;
                Ok(ControlFlow::Continue)
            }

            OpCode::CMP => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                let ord = match (a, b) {
                    (Value::Int(ai), Value::Int(bi)) => ai.cmp(&bi),
                    (Value::Float(af), Value::Float(bf)) => {
                        af.partial_cmp(&bf).unwrap_or(std::cmp::Ordering::Equal)
                    }
                    (Value::Int(ai), Value::Float(bf)) => (ai as f64)
                        .partial_cmp(&bf)
                        .unwrap_or(std::cmp::Ordering::Equal),
                    (Value::Float(af), Value::Int(bi)) => af
                        .partial_cmp(&(bi as f64))
                        .unwrap_or(std::cmp::Ordering::Equal),
                    (Value::String(a), Value::String(b)) => a.cmp(&b),
                    _ => {
                        return Err(VmError::TypeError(
                            "CMP expects comparable operands".to_string(),
                        ))
                    }
                };
                let result = match ord {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                };
                self.push_value(Value::Int(result))?;
                Ok(ControlFlow::Continue)
            }

            OpCode::IsNull => {
                let a = self.pop_value()?;
                self.push_value(Value::Bool(matches!(a, Value::Null)))?;
                Ok(ControlFlow::Continue)
            }

            OpCode::AND => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                self.push_value(Value::Bool(a.is_truthy() && b.is_truthy()))?;
                Ok(ControlFlow::Continue)
            }

            OpCode::OR => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                self.push_value(Value::Bool(a.is_truthy() || b.is_truthy()))?;
                Ok(ControlFlow::Continue)
            }

            OpCode::XOR => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                self.push_value(Value::Bool(a.is_truthy() ^ b.is_truthy()))?;
                Ok(ControlFlow::Continue)
            }

            OpCode::BAND => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                match (a, b) {
                    (Value::Int(ai), Value::Int(bi)) => {
                        self.push_value(Value::Int(ai & bi))?;
                        Ok(ControlFlow::Continue)
                    }
                    _ => Err(VmError::TypeError("BAND expects int operands".to_string())),
                }
            }

            OpCode::BOR => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                match (a, b) {
                    (Value::Int(ai), Value::Int(bi)) => {
                        self.push_value(Value::Int(ai | bi))?;
                        Ok(ControlFlow::Continue)
                    }
                    _ => Err(VmError::TypeError("BOR expects int operands".to_string())),
                }
            }

            OpCode::BNOT => {
                let a = self.pop_value()?;
                match a {
                    Value::Int(ai) => {
                        self.push_value(Value::Int(!ai))?;
                        Ok(ControlFlow::Continue)
                    }
                    _ => Err(VmError::TypeError("BNOT expects int operand".to_string())),
                }
            }

            OpCode::BXOR => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                match (a, b) {
                    (Value::Int(ai), Value::Int(bi)) => {
                        self.push_value(Value::Int(ai ^ bi))?;
                        Ok(ControlFlow::Continue)
                    }
                    _ => Err(VmError::TypeError("BXOR expects int operands".to_string())),
                }
            }

            OpCode::SHL => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                match (a, b) {
                    (Value::Int(ai), Value::Int(bi)) => {
                        let shift = (bi & 63) as u32;
                        self.push_value(Value::Int(ai << shift))?;
                        Ok(ControlFlow::Continue)
                    }
                    _ => Err(VmError::TypeError("SHL expects int operands".to_string())),
                }
            }

            OpCode::SHR => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                match (a, b) {
                    (Value::Int(ai), Value::Int(bi)) => {
                        let shift = (bi & 63) as u32;
                        self.push_value(Value::Int(ai >> shift))?;
                        Ok(ControlFlow::Continue)
                    }
                    _ => Err(VmError::TypeError("SHR expects int operands".to_string())),
                }
            }

            OpCode::USHR => {
                let b = self.pop_value()?;
                let a = self.pop_value()?;
                match (a, b) {
                    (Value::Int(ai), Value::Int(bi)) => {
                        let shift = (bi & 63) as u32;
                        let result = ((ai as u64) >> shift) as i64;
                        self.push_value(Value::Int(result))?;
                        Ok(ControlFlow::Continue)
                    }
                    _ => Err(VmError::TypeError("USHR expects int operands".to_string())),
                }
            }

            OpCode::JMP => {
                let target = self.operand_usize(&instr, 0, "jump target")?;
                let frame = self
                    .frames
                    .last_mut()
                    .ok_or_else(|| VmError::RuntimeError("No frames".to_string()))?;
                if target >= frame.function.instructions.len() {
                    return Err(VmError::InvalidOperand(format!("jump target {}", target)));
                }
                frame.ip = target;
                Ok(ControlFlow::Continue)
            }

            OpCode::JZ => {
                let cond = self.pop_value()?;
                let target = self.operand_usize(&instr, 0, "jump target")?;
                if !cond.is_truthy() {
                    let frame = self
                        .frames
                        .last_mut()
                        .ok_or_else(|| VmError::RuntimeError("No frames".to_string()))?;
                    if target >= frame.function.instructions.len() {
                        return Err(VmError::InvalidOperand(format!("jump target {}", target)));
                    }
                    frame.ip = target;
                }
                Ok(ControlFlow::Continue)
            }

            OpCode::JNZ => {
                let cond = self.pop_value()?;
                let target = self.operand_usize(&instr, 0, "jump target")?;
                if cond.is_truthy() {
                    let frame = self
                        .frames
                        .last_mut()
                        .ok_or_else(|| VmError::RuntimeError("No frames".to_string()))?;
                    if target >= frame.function.instructions.len() {
                        return Err(VmError::InvalidOperand(format!("jump target {}", target)));
                    }
                    frame.ip = target;
                }
                Ok(ControlFlow::Continue)
            }

            OpCode::RET => {
                let frame = self
                    .frames
                    .last()
                    .ok_or_else(|| VmError::RuntimeError("No frames".to_string()))?;
                let value = if self.stack.len() > frame.stack_base {
                    self.pop_value()?
                } else {
                    Value::Unit
                };
                Ok(ControlFlow::Return(value))
            }

            OpCode::CALL => {
                let func_raw = self.operand_i32(&instr, 0, "function index")?;
                let num_args = self.operand_usize(&instr, 1, "argument count")?;

                if num_args > self.stack.len() {
                    return Err(VmError::StackUnderflow);
                }

                let current = self
                    .frames
                    .last()
                    .ok_or_else(|| VmError::RuntimeError("No frames".to_string()))?;
                let module_name = current.module_name.clone();
                let function_idx = current.function_idx;
                if func_raw == -1 {
                    let callee = self.pop_value()?;
                    let callsite = (module_name.clone(), function_idx, instr_idx);
                    return self.call_dynamic(callee, num_args, &module_name, callsite);
                }

                if func_raw < 0 {
                    return Err(VmError::InvalidOperand(
                        "negative function index".to_string(),
                    ));
                }
                let func_idx = func_raw as usize;

                self.call_function_index(func_idx, num_args, &module_name)
            }

            OpCode::CallExt => {
                let name_idx = self.operand_usize(&instr, 0, "native name index")?;
                let num_args = self.operand_usize(&instr, 1, "argument count")?;
                if num_args > self.stack.len() {
                    return Err(VmError::StackUnderflow);
                }
                let frame = self
                    .frames
                    .last()
                    .ok_or_else(|| VmError::RuntimeError("No frames".to_string()))?;
                let native_name = self.resolve_constant_string(frame, name_idx, "native name")?;
                let native = self.resolve_native(&native_name)?;
                if native.arity != num_args {
                    return Err(VmError::InvalidOperand(format!(
                        "arity mismatch: expected {}, got {}",
                        native.arity, num_args
                    )));
                }
                let mut args = Vec::with_capacity(num_args);
                for _ in 0..num_args {
                    args.push(self.pop_value()?);
                }
                args.reverse();
                let result = (native.func)(&args).map_err(VmError::RuntimeError)?;
                self.push_value(result)?;
                Ok(ControlFlow::Continue)
            }

            OpCode::LOAD => {
                let idx = self.operand_usize(&instr, 0, "local index")?;
                let frame = self
                    .frames
                    .last()
                    .ok_or_else(|| VmError::RuntimeError("No frames".to_string()))?;
                let pos = frame.stack_base + idx;
                let value = self
                    .stack
                    .get(pos)
                    .ok_or_else(|| VmError::InvalidOperand(format!("local index {}", idx)))?
                    .clone();
                match value {
                    Value::Pointer(ptr) => {
                        let slot = self.heap.slots.get(ptr).and_then(|s| s.as_ref());
                        if let Some(slot) = slot {
                            if slot.is_upvalue {
                                self.push_value(slot.value.clone())?;
                                return Ok(ControlFlow::Continue);
                            }
                        }
                        self.push_value(Value::Pointer(ptr))?;
                    }
                    _ => {
                        self.push_value(value)?;
                    }
                }
                Ok(ControlFlow::Continue)
            }

            OpCode::STORE => {
                let idx = self.operand_usize(&instr, 0, "local index")?;
                let value = self.pop_value()?;
                let frame = self
                    .frames
                    .last()
                    .ok_or_else(|| VmError::RuntimeError("No frames".to_string()))?;
                let pos = frame.stack_base + idx;
                if pos >= self.stack.len() {
                    return Err(VmError::InvalidOperand(format!("local index {}", idx)));
                }
                match &self.stack[pos] {
                    Value::Pointer(ptr) => {
                        let slot = self.heap.slots.get_mut(*ptr).and_then(|s| s.as_mut());
                        if let Some(slot) = slot {
                            if slot.is_upvalue {
                                slot.value = value;
                                return Ok(ControlFlow::Continue);
                            }
                        }
                        self.stack[pos] = Value::Pointer(*ptr);
                    }
                    _ => {
                        self.stack[pos] = value;
                    }
                }
                Ok(ControlFlow::Continue)
            }

            OpCode::GetGlobal => {
                let idx = self.operand_usize(&instr, 0, "global name index")?;
                let frame = self
                    .frames
                    .last()
                    .ok_or_else(|| VmError::RuntimeError("No frames".to_string()))?;
                let name = self.resolve_constant_string(frame, idx, "global name")?;
                if let Some((version, value)) =
                    self.global_cache
                        .get(&(frame.module_name.clone(), frame.function_idx, idx))
                {
                    if *version == self.global_version {
                        self.push_value(value.clone())?;
                        return Ok(ControlFlow::Continue);
                    }
                }
                let value = self.globals.get(&name).cloned().unwrap_or(Value::Null);
                self.global_cache.insert(
                    (frame.module_name.clone(), frame.function_idx, idx),
                    (self.global_version, value.clone()),
                );
                self.push_value(value)?;
                Ok(ControlFlow::Continue)
            }

            OpCode::GetGlobalM => {
                let idx = self.operand_usize(&instr, 0, "global name index")?;
                let frame = self
                    .frames
                    .last()
                    .ok_or_else(|| VmError::RuntimeError("No frames".to_string()))?;
                let name = self.resolve_module_constant_string(frame, idx, "global name")?;
                let value = self.globals.get(&name).cloned().unwrap_or(Value::Null);
                self.push_value(value)?;
                Ok(ControlFlow::Continue)
            }

            OpCode::SetGlobal => {
                let idx = self.operand_usize(&instr, 0, "global name index")?;
                let value = self.stack.last().cloned().ok_or(VmError::StackUnderflow)?;
                let frame = self
                    .frames
                    .last()
                    .ok_or_else(|| VmError::RuntimeError("No frames".to_string()))?;
                let name = self.resolve_constant_string(frame, idx, "global name")?;
                self.globals.insert(name, value);
                self.global_version = self.global_version.saturating_add(1);
                Ok(ControlFlow::Continue)
            }

            OpCode::SetGlobalM => {
                let idx = self.operand_usize(&instr, 0, "global name index")?;
                let value = self.stack.last().cloned().ok_or(VmError::StackUnderflow)?;
                let frame = self
                    .frames
                    .last()
                    .ok_or_else(|| VmError::RuntimeError("No frames".to_string()))?;
                let name = self.resolve_module_constant_string(frame, idx, "global name")?;
                self.globals.insert(name, value);
                self.global_version = self.global_version.saturating_add(1);
                Ok(ControlFlow::Continue)
            }

            OpCode::ALLOC => {
                let size_hint = match instr.operand(0) {
                    Some(raw) => {
                        if raw < 0 {
                            return Err(VmError::InvalidOperand("negative alloc size".to_string()));
                        }
                        raw as usize
                    }
                    None => 0,
                };
                let value = self.pop_value()?;
                let ptr = self.heap_alloc(value, size_hint)?;
                self.push_value(Value::Pointer(ptr))?;
                Ok(ControlFlow::Continue)
            }

            OpCode::FREE => {
                let value = self.pop_value()?;
                match value {
                    Value::Pointer(ptr) => {
                        self.heap_free(ptr)?;
                        self.push_value(Value::Unit)?;
                        Ok(ControlFlow::Continue)
                    }
                    _ => Err(VmError::TypeError("FREE expects pointer".to_string())),
                }
            }

            OpCode::NewArray => {
                let len = self.operand_usize(&instr, 0, "array length")?;
                if len > self.stack.len() {
                    return Err(VmError::StackUnderflow);
                }
                let mut arr = Vec::with_capacity(len);
                for _ in 0..len {
                    arr.push(self.pop_value()?);
                }
                arr.reverse();
                self.push_value(Value::Array(arr))?;
                Ok(ControlFlow::Continue)
            }

            OpCode::PushM => {
                let idx = self.operand_usize(&instr, 0, "module constant index")?;
                let frame = self
                    .frames
                    .last()
                    .ok_or_else(|| VmError::RuntimeError("No frames".to_string()))?;
                let module = self.modules.get(&frame.module_name).ok_or_else(|| {
                    VmError::RuntimeError(format!("Module not found: {}", frame.module_name))
                })?;
                let value = module
                    .constants
                    .get(idx)
                    .ok_or_else(|| {
                        VmError::InvalidOperand(format!("module constant index {}", idx))
                    })?
                    .clone();
                self.push_value(value)?;
                Ok(ControlFlow::Continue)
            }

            OpCode::CLOSURE => {
                let func_idx = self.operand_usize(&instr, 0, "function index")?;
                let num_upvalues = self.operand_usize(&instr, 1, "upvalue count")?;
                if num_upvalues > self.stack.len() {
                    return Err(VmError::StackUnderflow);
                }
                let upvalue_names = {
                    let current = self
                        .frames
                        .last()
                        .ok_or_else(|| VmError::RuntimeError("No frames".to_string()))?;
                    let module = self.modules.get(&current.module_name).ok_or_else(|| {
                        VmError::RuntimeError(format!("Module not found: {}", current.module_name))
                    })?;
                    let func = module.get_function(func_idx).ok_or_else(|| {
                        VmError::UndefinedFunction(format!("Function index: {}", func_idx))
                    })?;
                    if func.upvalues.len() != num_upvalues {
                        return Err(VmError::InvalidOperand(format!(
                            "upvalue count mismatch: expected {}, got {}",
                            func.upvalues.len(),
                            num_upvalues
                        )));
                    }
                    func.upvalues.clone()
                };
                let mut upvalues = Vec::with_capacity(num_upvalues);
                for _ in 0..num_upvalues {
                    upvalues.push(self.pop_value()?);
                }
                upvalues.reverse();
                let upvalue_captures = vec![crate::bytecode::UpvalueCapture::ByValue; num_upvalues];
                self.push_value(Value::Closure(crate::bytecode::Closure {
                    function_idx: func_idx,
                    upvalues,
                    upvalue_names,
                    upvalue_captures,
                }))?;
                Ok(ControlFlow::Continue)
            }

            OpCode::ClosureRef => {
                let func_idx = self.operand_usize(&instr, 0, "function index")?;
                let num_upvalues = self.operand_usize(&instr, 1, "upvalue count")?;
                let expected = 2 + num_upvalues;
                if instr.operands.len() < expected {
                    return Err(VmError::InvalidOperand(
                        "missing upvalue indices".to_string(),
                    ));
                }
                let frame = self
                    .frames
                    .last()
                    .ok_or_else(|| VmError::RuntimeError("No frames".to_string()))?
                    .clone();
                let upvalue_names = {
                    let module = self.modules.get(&frame.module_name).ok_or_else(|| {
                        VmError::RuntimeError(format!("Module not found: {}", frame.module_name))
                    })?;
                    let func = module.get_function(func_idx).ok_or_else(|| {
                        VmError::UndefinedFunction(format!("Function index: {}", func_idx))
                    })?;
                    if func.upvalues.len() != num_upvalues {
                        return Err(VmError::InvalidOperand(format!(
                            "upvalue count mismatch: expected {}, got {}",
                            func.upvalues.len(),
                            num_upvalues
                        )));
                    }
                    func.upvalues.clone()
                };
                let mut upvalues = Vec::with_capacity(num_upvalues);
                let mut upvalue_captures = Vec::with_capacity(num_upvalues);
                for i in 0..num_upvalues {
                    let local_idx = self.operand_usize(&instr, 2 + i, "upvalue local")?;
                    let ptr = self.ensure_upvalue_cell(&frame, local_idx)?;
                    upvalues.push(Value::Pointer(ptr));
                    upvalue_captures
                        .push(crate::bytecode::UpvalueCapture::ByRefLocal { local_idx });
                }
                self.push_value(Value::Closure(crate::bytecode::Closure {
                    function_idx: func_idx,
                    upvalues,
                    upvalue_names,
                    upvalue_captures,
                }))?;
                Ok(ControlFlow::Continue)
            }

            OpCode::ClosureRefStack => {
                let func_idx = self.operand_usize(&instr, 0, "function index")?;
                let num_upvalues = self.operand_usize(&instr, 1, "upvalue count")?;
                if num_upvalues > self.stack.len() {
                    return Err(VmError::StackUnderflow);
                }
                let upvalue_names = {
                    let frame = self
                        .frames
                        .last()
                        .ok_or_else(|| VmError::RuntimeError("No frames".to_string()))?;
                    let module = self.modules.get(&frame.module_name).ok_or_else(|| {
                        VmError::RuntimeError(format!("Module not found: {}", frame.module_name))
                    })?;
                    let func = module.get_function(func_idx).ok_or_else(|| {
                        VmError::UndefinedFunction(format!("Function index: {}", func_idx))
                    })?;
                    if func.upvalues.len() != num_upvalues {
                        return Err(VmError::InvalidOperand(format!(
                            "upvalue count mismatch: expected {}, got {}",
                            func.upvalues.len(),
                            num_upvalues
                        )));
                    }
                    func.upvalues.clone()
                };
                let mut upvalues = Vec::with_capacity(num_upvalues);
                for _ in 0..num_upvalues {
                    let captured = self.pop_value()?;
                    let ptr = match captured {
                        Value::Pointer(ptr) => {
                            let slot = self.heap.slots.get(ptr).and_then(|s| s.as_ref());
                            if let Some(slot) = slot {
                                if slot.is_upvalue {
                                    ptr
                                } else {
                                    self.heap_alloc_upvalue(Value::Pointer(ptr))?
                                }
                            } else {
                                self.heap_alloc_upvalue(Value::Null)?
                            }
                        }
                        value => self.heap_alloc_upvalue(value)?,
                    };
                    upvalues.push(Value::Pointer(ptr));
                }
                upvalues.reverse();
                let upvalue_captures = (0..num_upvalues)
                    .map(|i| crate::bytecode::UpvalueCapture::ByRefStack { stack_index: i })
                    .collect();
                self.push_value(Value::Closure(crate::bytecode::Closure {
                    function_idx: func_idx,
                    upvalues,
                    upvalue_names,
                    upvalue_captures,
                }))?;
                Ok(ControlFlow::Continue)
            }

            OpCode::NewObj => {
                let pairs = self.operand_usize(&instr, 0, "object pair count")?;
                if pairs * 2 > self.stack.len() {
                    return Err(VmError::StackUnderflow);
                }
                let mut obj = HashMap::new();
                for _ in 0..pairs {
                    let value = self.pop_value()?;
                    let key = self.pop_value()?;
                    let key_str = match key {
                        Value::String(s) => s,
                        _ => {
                            return Err(VmError::TypeError("object key must be string".to_string()))
                        }
                    };
                    obj.insert(key_str, value);
                }
                self.push_value(Value::Object(obj))?;
                Ok(ControlFlow::Continue)
            }

            OpCode::GetField => {
                let field = self.pop_value()?;
                let target = self.pop_value()?;
                let field = match field {
                    Value::String(s) => s,
                    _ => return Err(VmError::TypeError("field name must be string".to_string())),
                };
                match target {
                    Value::Pointer(ptr) => {
                        if let Some(slot) = self.heap.slots.get(ptr).and_then(|s| s.as_ref()) {
                            if let Some((gen, ver, value)) =
                                self.field_cache.get(&(ptr, field.clone()))
                            {
                                if *gen == slot.generation && *ver == slot.version {
                                    self.push_value(value.clone())?;
                                    return Ok(ControlFlow::Continue);
                                }
                            }
                        }
                        let resolved = self.heap_get(ptr)?.clone();
                        match resolved {
                            Value::Object(obj) => {
                                let value = obj
                                    .get(&field)
                                    .cloned()
                                    .ok_or_else(|| VmError::UndefinedVariable(field.clone()))?;
                                if let Some(slot) =
                                    self.heap.slots.get(ptr).and_then(|s| s.as_ref())
                                {
                                    self.field_cache.insert(
                                        (ptr, field),
                                        (slot.generation, slot.version, value.clone()),
                                    );
                                }
                                self.push_value(value)?;
                                Ok(ControlFlow::Continue)
                            }
                            _ => Err(VmError::TypeError("GetField expects object".to_string())),
                        }
                    }
                    _ => {
                        let resolved = target;
                        match resolved {
                            Value::Object(obj) => {
                                let value = obj
                                    .get(&field)
                                    .cloned()
                                    .ok_or_else(|| VmError::UndefinedVariable(field.clone()))?;
                                self.push_value(value)?;
                                Ok(ControlFlow::Continue)
                            }
                            _ => Err(VmError::TypeError("GetField expects object".to_string())),
                        }
                    }
                }
            }

            OpCode::SetField => {
                let value = self.pop_value()?;
                let field = self.pop_value()?;
                let target = self.pop_value()?;
                let field = match field {
                    Value::String(s) => s,
                    _ => return Err(VmError::TypeError("field name must be string".to_string())),
                };
                match target {
                    Value::Pointer(ptr) => {
                        let obj = self.heap_get_mut(ptr)?;
                        match obj {
                            Value::Object(map) => {
                                map.insert(field, value);
                                if let Some(slot) =
                                    self.heap.slots.get_mut(ptr).and_then(|s| s.as_mut())
                                {
                                    slot.version = slot.version.saturating_add(1);
                                }
                                self.push_value(Value::Pointer(ptr))?;
                                Ok(ControlFlow::Continue)
                            }
                            _ => Err(VmError::TypeError("SetField expects object".to_string())),
                        }
                    }
                    Value::Object(mut map) => {
                        map.insert(field, value);
                        // Autobox mutated objects onto the VM heap so subsequent accesses can use pointer caches.
                        let ptr = self.heap_alloc(Value::Object(map), 0)?;
                        self.push_value(Value::Pointer(ptr))?;
                        Ok(ControlFlow::Continue)
                    }
                    _ => Err(VmError::TypeError("SetField expects object".to_string())),
                }
            }

            OpCode::GetIndex => {
                let index = self.pop_value()?;
                let target = self.pop_value()?;
                let idx = match index {
                    Value::Int(i) => i,
                    _ => return Err(VmError::TypeError("index must be int".to_string())),
                };
                match target {
                    Value::Pointer(ptr) => {
                        if let Some(slot) = self.heap.slots.get(ptr).and_then(|s| s.as_ref()) {
                            if let Some((gen, ver, value)) = self.index_cache.get(&(ptr, idx)) {
                                if *gen == slot.generation && *ver == slot.version {
                                    self.push_value(value.clone())?;
                                    return Ok(ControlFlow::Continue);
                                }
                            }
                        }
                        let resolved = self.heap_get(ptr)?.clone();
                        match resolved {
                            Value::Array(arr) => {
                                let idx = idx as isize;
                                if idx < 0 || idx as usize >= arr.len() {
                                    return Err(VmError::InvalidOperand(
                                        "index out of bounds".to_string(),
                                    ));
                                }
                                let value = arr[idx as usize].clone();
                                if let Some(slot) =
                                    self.heap.slots.get(ptr).and_then(|s| s.as_ref())
                                {
                                    self.index_cache.insert(
                                        (ptr, idx as i64),
                                        (slot.generation, slot.version, value.clone()),
                                    );
                                }
                                self.push_value(value)?;
                                Ok(ControlFlow::Continue)
                            }
                            Value::String(s) => {
                                let idx = idx as isize;
                                if idx < 0 {
                                    return Err(VmError::InvalidOperand(
                                        "index out of bounds".to_string(),
                                    ));
                                }
                                let ch = s.chars().nth(idx as usize).ok_or_else(|| {
                                    VmError::InvalidOperand("index out of bounds".to_string())
                                })?;
                                self.push_value(Value::String(ch.to_string()))?;
                                Ok(ControlFlow::Continue)
                            }
                            _ => Err(VmError::TypeError(
                                "GetIndex expects array or string".to_string(),
                            )),
                        }
                    }
                    _ => {
                        let resolved = target;
                        match resolved {
                            Value::Array(arr) => {
                                let idx = idx as isize;
                                if idx < 0 || idx as usize >= arr.len() {
                                    return Err(VmError::InvalidOperand(
                                        "index out of bounds".to_string(),
                                    ));
                                }
                                self.push_value(arr[idx as usize].clone())?;
                                Ok(ControlFlow::Continue)
                            }
                            Value::String(s) => {
                                let idx = idx as isize;
                                if idx < 0 {
                                    return Err(VmError::InvalidOperand(
                                        "index out of bounds".to_string(),
                                    ));
                                }
                                let ch = s.chars().nth(idx as usize).ok_or_else(|| {
                                    VmError::InvalidOperand("index out of bounds".to_string())
                                })?;
                                self.push_value(Value::String(ch.to_string()))?;
                                Ok(ControlFlow::Continue)
                            }
                            _ => Err(VmError::TypeError(
                                "GetIndex expects array or string".to_string(),
                            )),
                        }
                    }
                }
            }

            OpCode::SetIndex => {
                let value = self.pop_value()?;
                let index = self.pop_value()?;
                let target = self.pop_value()?;
                let idx = match index {
                    Value::Int(i) => i,
                    _ => return Err(VmError::TypeError("index must be int".to_string())),
                };
                match target {
                    Value::Pointer(ptr) => {
                        let obj = self.heap_get_mut(ptr)?;
                        match obj {
                            Value::Array(arr) => {
                                let idx = idx as isize;
                                if idx < 0 || idx as usize >= arr.len() {
                                    return Err(VmError::InvalidOperand(
                                        "index out of bounds".to_string(),
                                    ));
                                }
                                arr[idx as usize] = value;
                                if let Some(slot) =
                                    self.heap.slots.get_mut(ptr).and_then(|s| s.as_mut())
                                {
                                    slot.version = slot.version.saturating_add(1);
                                }
                                self.push_value(Value::Pointer(ptr))?;
                                Ok(ControlFlow::Continue)
                            }
                            _ => Err(VmError::TypeError("SetIndex expects array".to_string())),
                        }
                    }
                    Value::Array(mut arr) => {
                        let idx = idx as isize;
                        if idx < 0 || idx as usize >= arr.len() {
                            return Err(VmError::InvalidOperand("index out of bounds".to_string()));
                        }
                        arr[idx as usize] = value;
                        // Autobox mutated arrays onto the VM heap so subsequent accesses can use pointer caches.
                        let ptr = self.heap_alloc(Value::Array(arr), 0)?;
                        self.push_value(Value::Pointer(ptr))?;
                        Ok(ControlFlow::Continue)
                    }
                    _ => Err(VmError::TypeError("SetIndex expects array".to_string())),
                }
            }

            OpCode::LEN => {
                let value = self.pop_value()?;
                let resolved = match value {
                    Value::Pointer(ptr) => self.heap_get(ptr)?.clone(),
                    _ => value,
                };
                let len = match resolved {
                    Value::String(s) => s.chars().count() as i64,
                    Value::Array(arr) => arr.len() as i64,
                    Value::Object(obj) => obj.len() as i64,
                    _ => 0,
                };
                self.push_value(Value::Int(len))?;
                Ok(ControlFlow::Continue)
            }

            OpCode::SLICE => {
                let end = self.pop_value()?;
                let start = self.pop_value()?;
                let target = self.pop_value()?;
                let start = match start {
                    Value::Int(i) => i,
                    _ => return Err(VmError::TypeError("slice start must be int".to_string())),
                };
                let end = match end {
                    Value::Int(i) => i,
                    _ => return Err(VmError::TypeError("slice end must be int".to_string())),
                };
                let resolved = match target {
                    Value::Pointer(ptr) => self.heap_get(ptr)?.clone(),
                    _ => target,
                };
                match resolved {
                    Value::Array(arr) => {
                        let len = arr.len() as isize;
                        let mut s = start as isize;
                        let mut e = end as isize;
                        if s < 0 {
                            s += len;
                        }
                        if e < 0 {
                            e += len;
                        }
                        if s < 0 || e < s || e > len {
                            return Err(VmError::InvalidOperand("slice out of bounds".to_string()));
                        }
                        let slice = arr[s as usize..e as usize].to_vec();
                        self.push_value(Value::Array(slice))?;
                        Ok(ControlFlow::Continue)
                    }
                    Value::String(s) => {
                        let chars: Vec<char> = s.chars().collect();
                        let len = chars.len() as isize;
                        let mut s_idx = start as isize;
                        let mut e_idx = end as isize;
                        if s_idx < 0 {
                            s_idx += len;
                        }
                        if e_idx < 0 {
                            e_idx += len;
                        }
                        if s_idx < 0 || e_idx < s_idx || e_idx > len {
                            return Err(VmError::InvalidOperand("slice out of bounds".to_string()));
                        }
                        let slice: String = chars[s_idx as usize..e_idx as usize].iter().collect();
                        self.push_value(Value::String(slice))?;
                        Ok(ControlFlow::Continue)
                    }
                    _ => Err(VmError::TypeError(
                        "SLICE expects array or string".to_string(),
                    )),
                }
            }

            OpCode::CONTAINS => {
                let needle = self.pop_value()?;
                let haystack = self.pop_value()?;
                let resolved_haystack = match haystack {
                    Value::Pointer(ptr) => self.heap_get(ptr)?.clone(),
                    _ => haystack,
                };
                match (resolved_haystack, needle) {
                    (Value::String(h), Value::String(n)) => {
                        self.push_value(Value::Bool(h.contains(&n)))?;
                        Ok(ControlFlow::Continue)
                    }
                    (Value::Array(arr), n) => {
                        self.push_value(Value::Bool(arr.contains(&n)))?;
                        Ok(ControlFlow::Continue)
                    }
                    _ => Err(VmError::TypeError(
                        "CONTAINS expects (string, string) or (array, any)".to_string(),
                    )),
                }
            }

            OpCode::SPLIT => {
                let delimiter = self.pop_value()?;
                let input = self.pop_value()?;
                let resolved_input = match input {
                    Value::Pointer(ptr) => self.heap_get(ptr)?.clone(),
                    _ => input,
                };
                match (resolved_input, delimiter) {
                    (Value::String(s), Value::String(d)) => {
                        let parts: Vec<Value> =
                            s.split(&d).map(|p| Value::String(p.to_string())).collect();
                        self.push_value(Value::Array(parts))?;
                        Ok(ControlFlow::Continue)
                    }
                    _ => Err(VmError::TypeError(
                        "SPLIT expects (string, string)".to_string(),
                    )),
                }
            }

            OpCode::CHARS => {
                let input = self.pop_value()?;
                let resolved_input = match input {
                    Value::Pointer(ptr) => self.heap_get(ptr)?.clone(),
                    _ => input,
                };
                match resolved_input {
                    Value::String(s) => {
                        let chars: Vec<Value> =
                            s.chars().map(|c| Value::String(c.to_string())).collect();
                        self.push_value(Value::Array(chars))?;
                        Ok(ControlFlow::Continue)
                    }
                    _ => Err(VmError::TypeError("CHARS expects string".to_string())),
                }
            }

            OpCode::SHIFT => {
                let target = self.pop_value()?;
                match target {
                    Value::Pointer(ptr) => {
                        let obj = self.heap_get_mut(ptr)?;
                        match obj {
                            Value::Array(arr) => {
                                if arr.is_empty() {
                                    self.push_value(Value::Null)?;
                                } else {
                                    let first = arr.remove(0);
                                    if let Some(slot) =
                                        self.heap.slots.get_mut(ptr).and_then(|s| s.as_mut())
                                    {
                                        slot.version = slot.version.saturating_add(1);
                                    }
                                    self.push_value(first)?;
                                }
                                Ok(ControlFlow::Continue)
                            }
                            _ => Err(VmError::TypeError(
                                "SHIFT expects array pointer".to_string(),
                            )),
                        }
                    }
                    Value::Array(mut arr) => {
                        if arr.is_empty() {
                            self.push_value(Value::Null)?;
                        } else {
                            let first = arr.remove(0);
                            // Push the remaining array back to stack?
                            // Usually SHIFT modifies the source. If it's on stack, we just return the element.
                            // But wait, if someone does `[1,2].shift()`, they expect 1.
                            // If they did `let a = [1,2]; a.shift()`, they expect `a` to be `[2]`.
                            // If it's on stack, the array is effectively "consumed".
                            self.push_value(first)?;
                        }
                        Ok(ControlFlow::Continue)
                    }
                    _ => Err(VmError::TypeError(
                        "SHIFT expects array or array pointer".to_string(),
                    )),
                }
            }
        }
    }

    /// Binary operation on values
    fn binary_op<F>(&self, a: &Value, b: &Value, op: F) -> VmResult<Value>
    where
        F: FnOnce(f64, f64) -> f64,
    {
        match (a, b) {
            (Value::Int(ai), Value::Int(bi)) => {
                let result = op(*ai as f64, *bi as f64) as i64;
                Ok(Value::Int(result))
            }
            (Value::Float(af), Value::Float(bf)) => Ok(Value::Float(op(*af, *bf))),
            (Value::Int(ai), Value::Float(bf)) => Ok(Value::Float(op(*ai as f64, *bf))),
            (Value::Float(af), Value::Int(bi)) => Ok(Value::Float(op(*af, *bi as f64))),
            _ => Err(VmError::TypeError(
                "Invalid operands for binary operation".to_string(),
            )),
        }
    }

    /// Unary operation on values
    fn unary_op<F>(&self, a: &Value, op: F) -> VmResult<Value>
    where
        F: FnOnce(f64) -> f64,
    {
        match a {
            Value::Int(ai) => Ok(Value::Int(op(*ai as f64) as i64)),
            Value::Float(af) => Ok(Value::Float(op(*af))),
            _ => Err(VmError::TypeError(
                "Invalid operand for unary operation".to_string(),
            )),
        }
    }

    /// Comparison operation
    fn compare_op<F>(&self, a: &Value, b: &Value, op: F) -> VmResult<bool>
    where
        F: FnOnce(f64, f64) -> bool,
    {
        match (a, b) {
            (Value::Int(ai), Value::Int(bi)) => Ok(op(*ai as f64, *bi as f64)),
            (Value::Float(af), Value::Float(bf)) => Ok(op(*af, *bf)),
            (Value::Int(ai), Value::Float(bf)) => Ok(op(*ai as f64, *bf)),
            (Value::Float(af), Value::Int(bi)) => Ok(op(*af, *bi as f64)),
            _ => Err(VmError::TypeError(
                "Invalid operands for comparison".to_string(),
            )),
        }
    }

    /// Check if value is zero
    fn is_zero(&self, value: &Value) -> bool {
        match value {
            Value::Int(i) => *i == 0,
            Value::Float(f) => *f == 0.0,
            _ => false,
        }
    }

    /// Get instruction count
    pub fn instruction_count(&self) -> u64 {
        self.instruction_count
    }
}

/// Control flow for execution
#[derive(Debug)]
pub enum ControlFlow {
    Continue,
    Return(Value),
    Breakpoint(u32),
}

#[derive(Debug, Clone)]
enum CallCacheEntry {
    Function(usize),
    Native(String),
    Closure(usize, usize),
}

/// Nyx VM main entry point
pub struct NyxVm {
    runtime: VmRuntime,
}

impl NyxVm {
    /// Create new VM
    pub fn new(config: VmConfig) -> Self {
        let runtime = VmRuntime::new(config);
        Self { runtime }
    }

    /// Load bytecode module
    pub fn load(&mut self, module: BytecodeModule) {
        self.runtime.load_module(module);
    }

    /// Register native function
    pub fn register(
        &mut self,
        name: &str,
        arity: usize,
        func: fn(&[Value]) -> Result<Value, String>,
    ) {
        self.runtime.register_native(name, arity, func);
    }

    /// Run the VM
    pub fn run(&mut self, module_name: &str) -> VmResult<Value> {
        self.runtime.run(module_name)
    }

    /// Run specific function
    pub fn run_function(
        &mut self,
        module_name: &str,
        func_idx: usize,
        args: Vec<Value>,
    ) -> VmResult<Value> {
        self.runtime.run_function(module_name, func_idx, args)
    }

    /// Access the underlying runtime state
    pub fn runtime(&self) -> &VmRuntime {
        &self.runtime
    }

    /// Access the underlying runtime state mutably
    pub fn runtime_mut(&mut self) -> &mut VmRuntime {
        &mut self.runtime
    }

    /// Get total instruction count
    pub fn instruction_count(&self) -> u64 {
        self.runtime.instruction_count
    }

    /// Get total bytes allocated (deterministic)
    pub fn total_allocated(&self) -> u64 {
        self.runtime.heap.total_allocated
    }

    /// Get total bytes freed (deterministic)
    pub fn total_freed(&self) -> u64 {
        self.runtime.heap.total_freed
    }
}

impl Default for NyxVm {
    fn default() -> Self {
        Self::new(VmConfig::default())
    }
}

// -------------------------------------------------------------------------------------------------
// VM-Aware JIT Runtime Stubs
//
// These `extern "C"` functions are called from Cranelift-generated code when the `jit` feature is
// enabled. They must never panic and must never cause UB even if invoked with bad operands.
//
// Convention:
// - Return `0` on success.
// - Return `-1` on error and store the corresponding `VmError` in `VmRuntime.jit_trap`.
// - For boolean-producing helpers, return `0/1` for false/true and `-1` for error.
// -------------------------------------------------------------------------------------------------

#[cfg(feature = "jit")]
fn jit_err(rt: &mut VmRuntime, err: VmError) -> i64 {
    rt.jit_set_trap(err);
    -1
}

#[cfg(feature = "jit")]
unsafe fn jit_wrap(rt: *mut VmRuntime, f: impl FnOnce(&mut VmRuntime) -> i64) -> i64 {
    let Some(rt) = rt.as_mut() else { return -1 };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(rt))) {
        Ok(v) => v,
        Err(_) => jit_err(
            rt,
            VmError::RuntimeError("panic in JIT runtime stub".to_string()),
        ),
    }
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_stack_guard(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        if rt.frames.len() >= rt.config.max_call_depth {
            return jit_err(rt, VmError::StackOverflow);
        }
        if rt.stack.len() + 100 > rt.config.max_stack_size {
            // Safety buffer
            return jit_err(rt, VmError::StackOverflow);
        }
        0
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_check_arity(rt: *mut VmRuntime, expected: i32) -> i64 {
    jit_wrap(rt, |rt| {
        if rt.stack.len() < expected as usize {
            return jit_err(
                rt,
                VmError::TypeError(format!(
                    "arity mismatch: expected {}, but stack size is only {}",
                    expected,
                    rt.stack.len()
                )),
            );
        }
        0
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_halt(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        rt.jit_set_retval(Value::Unit);
        0
    })
}
#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_tick(rt: *mut VmRuntime, ip: i32) -> i64 {
    jit_wrap(rt, |rt| {
        if let Some(frame) = rt.frames.last_mut() {
            frame.ip = ip as usize;
        }
        rt.instruction_count = rt.instruction_count.saturating_add(1);
        if rt.instruction_count % 1 == 0 {
            println!(
                "JIT TICK: ip={}, i_count={}, stack_len={}",
                ip,
                rt.instruction_count,
                rt.stack.len()
            );
        }

        match rt.check_limits() {
            Ok(()) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_pop_truthy(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| match rt.pop_value() {
        Ok(v) => {
            if v.is_truthy() {
                1
            } else {
                0
            }
        }
        Err(e) => jit_err(rt, e),
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_push_const(rt: *mut VmRuntime, idx: i32) -> i64 {
    jit_wrap(rt, |rt| {
        if idx < 0 {
            return jit_err(
                rt,
                VmError::InvalidOperand("negative constant index".to_string()),
            );
        }
        let Some(frame) = rt.frames.last() else {
            return jit_err(rt, VmError::RuntimeError("No frames".to_string()));
        };
        match rt
            .resolve_constant(frame, idx as usize)
            .and_then(|v| rt.push_value(v))
        {
            Ok(()) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_pushm(rt: *mut VmRuntime, idx: i32) -> i64 {
    jit_wrap(rt, |rt| {
        if idx < 0 {
            return jit_err(
                rt,
                VmError::InvalidOperand("negative module constant index".to_string()),
            );
        }
        let Some(frame) = rt.frames.last() else {
            return jit_err(rt, VmError::RuntimeError("No frames".to_string()));
        };
        let Some(module) = rt.modules.get(&frame.module_name) else {
            return jit_err(
                rt,
                VmError::RuntimeError(format!("Module not found: {}", frame.module_name)),
            );
        };
        match module
            .constants
            .get(idx as usize)
            .cloned()
            .ok_or_else(|| VmError::InvalidOperand(format!("module constant index {}", idx)))
            .and_then(|v| rt.push_value(v))
        {
            Ok(()) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_pop(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| match rt.pop_value() {
        Ok(_) => 0,
        Err(e) => jit_err(rt, e),
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_dup(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        let top = match rt.stack.last().cloned() {
            Some(v) => v,
            None => return jit_err(rt, VmError::StackUnderflow),
        };
        match rt.push_value(top) {
            Ok(()) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_swap(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        if rt.stack.len() < 2 {
            return jit_err(rt, VmError::StackUnderflow);
        }
        let len = rt.stack.len();
        rt.stack.swap(len - 1, len - 2);
        0
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_load(rt: *mut VmRuntime, local_idx: i32) -> i64 {
    jit_wrap(rt, |rt| {
        if local_idx < 0 {
            return jit_err(
                rt,
                VmError::InvalidOperand("negative local index".to_string()),
            );
        }
        let Some(frame) = rt.frames.last() else {
            return jit_err(rt, VmError::RuntimeError("No frames".to_string()));
        };
        let pos = frame.stack_base + (local_idx as usize);
        let value = match rt.stack.get(pos).cloned() {
            Some(v) => v,
            None => {
                return jit_err(
                    rt,
                    VmError::InvalidOperand(format!("local index {}", local_idx)),
                )
            }
        };

        // eprintln!("JIT_LOAD: idx={}, val={:?}", local_idx, value);

        // Mirror `LOAD` behavior (upvalue pointer deref).
        match value {
            Value::Pointer(ptr) => {
                let slot = rt.heap.slots.get(ptr).and_then(|s| s.as_ref());
                if let Some(slot) = slot {
                    if slot.is_upvalue {
                        return match rt.push_value(slot.value.clone()) {
                            Ok(()) => 0,
                            Err(e) => jit_err(rt, e),
                        };
                    }
                }
                match rt.push_value(Value::Pointer(ptr)) {
                    Ok(()) => 0,
                    Err(e) => jit_err(rt, e),
                }
            }
            _ => match rt.push_value(value) {
                Ok(()) => 0,
                Err(e) => jit_err(rt, e),
            },
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_store(rt: *mut VmRuntime, local_idx: i32) -> i64 {
    jit_wrap(rt, |rt| {
        if local_idx < 0 {
            return jit_err(
                rt,
                VmError::InvalidOperand("negative local index".to_string()),
            );
        }
        let value = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let Some(frame) = rt.frames.last() else {
            return jit_err(rt, VmError::RuntimeError("No frames".to_string()));
        };
        let pos = frame.stack_base + (local_idx as usize);
        if pos >= rt.stack.len() {
            return jit_err(
                rt,
                VmError::InvalidOperand(format!("local index {}", local_idx)),
            );
        }

        match &rt.stack[pos] {
            Value::Pointer(ptr) => {
                let slot = rt.heap.slots.get_mut(*ptr).and_then(|s| s.as_mut());
                if let Some(slot) = slot {
                    slot.value = value.clone();
                }
            }
            _ => {
                rt.stack[pos] = value.clone();
            }
        }
        0
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_add(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        let b = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let a = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let result = match (&a, &b) {
            (Value::String(s1), Value::String(s2)) => Ok(Value::String(format!("{}{}", s1, s2))),
            _ => rt.binary_op(&a, &b, |x, y| x + y),
        };
        match result.and_then(|v| rt.push_value(v)) {
            Ok(()) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_sub(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        let b = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let a = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        match rt
            .binary_op(&a, &b, |x, y| x - y)
            .and_then(|v| rt.push_value(v))
        {
            Ok(()) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_mul(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        let b = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let a = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        match rt
            .binary_op(&a, &b, |x, y| x * y)
            .and_then(|v| rt.push_value(v))
        {
            Ok(()) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_div(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        let b = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let a = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        // Preserve interpreter behavior: division by zero is an error.
        match &b {
            Value::Int(0) => return jit_err(rt, VmError::DivisionByZero),
            Value::Float(f) if *f == 0.0 => return jit_err(rt, VmError::DivisionByZero),
            _ => {}
        }
        match rt
            .binary_op(&a, &b, |x, y| x / y)
            .and_then(|v| rt.push_value(v))
        {
            Ok(()) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_neg(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        let a = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        match rt.unary_op(&a, |x| -x).and_then(|v| rt.push_value(v)) {
            Ok(()) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_eq(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        let b = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let a = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        match rt
            .compare_op(&a, &b, |x, y| x == y)
            .and_then(|b| rt.push_value(Value::Bool(b)))
        {
            Ok(()) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_ne(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        let b = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let a = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        match rt
            .compare_op(&a, &b, |x, y| x != y)
            .and_then(|b| rt.push_value(Value::Bool(b)))
        {
            Ok(()) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_lt(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        let b = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let a = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        match rt
            .compare_op(&a, &b, |x, y| x < y)
            .and_then(|b| rt.push_value(Value::Bool(b)))
        {
            Ok(()) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_gt(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        let b = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let a = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        match rt
            .compare_op(&a, &b, |x, y| x > y)
            .and_then(|b| rt.push_value(Value::Bool(b)))
        {
            Ok(()) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_le(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        let b = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let a = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        match rt
            .compare_op(&a, &b, |x, y| x <= y)
            .and_then(|b| rt.push_value(Value::Bool(b)))
        {
            Ok(()) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_ge(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        let b = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let a = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        match rt
            .compare_op(&a, &b, |x, y| x >= y)
            .and_then(|b| rt.push_value(Value::Bool(b)))
        {
            Ok(()) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_new_array(rt: *mut VmRuntime, len: i32) -> i64 {
    jit_wrap(rt, |rt| {
        if len < 0 {
            return jit_err(
                rt,
                VmError::InvalidOperand("negative array length".to_string()),
            );
        }
        let len = len as usize;
        if len > rt.stack.len() {
            return jit_err(rt, VmError::StackUnderflow);
        }
        let mut arr = Vec::with_capacity(len);
        for _ in 0..len {
            match rt.pop_value() {
                Ok(v) => arr.push(v),
                Err(e) => return jit_err(rt, e),
            }
        }
        arr.reverse();
        match rt.push_value(Value::Array(arr)) {
            Ok(()) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_new_obj(rt: *mut VmRuntime, pairs: i32) -> i64 {
    jit_wrap(rt, |rt| {
        if pairs < 0 {
            return jit_err(
                rt,
                VmError::InvalidOperand("negative object pair count".to_string()),
            );
        }
        let pairs = pairs as usize;
        if pairs * 2 > rt.stack.len() {
            return jit_err(rt, VmError::StackUnderflow);
        }
        let mut obj = HashMap::new();
        for _ in 0..pairs {
            let value = match rt.pop_value() {
                Ok(v) => v,
                Err(e) => return jit_err(rt, e),
            };
            let key = match rt.pop_value() {
                Ok(v) => v,
                Err(e) => return jit_err(rt, e),
            };
            let key_str = match key {
                Value::String(s) => s,
                _ => {
                    return jit_err(
                        rt,
                        VmError::TypeError("object key must be string".to_string()),
                    )
                }
            };
            obj.insert(key_str, value);
        }
        match rt.push_value(Value::Object(obj)) {
            Ok(()) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
#[repr(C)]
pub struct JitHeapInfo {
    pub slots_ptr: *const *const HeapSlot,
    pub slots_len: usize,
}

#[cfg(feature = "jit")]
#[repr(C)]
pub struct JitStackInfo {
    pub ptr: *mut Value,
    pub len: usize,
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_get_heap_info(rt: *const VmRuntime) -> JitHeapInfo {
    let rt = &*rt;
    JitHeapInfo {
        slots_ptr: rt.heap.slots.as_ptr() as *const *const HeapSlot,
        slots_len: rt.heap.slots.len(),
    }
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_get_stack_info(rt: *const VmRuntime) -> JitStackInfo {
    let rt = &*rt;
    JitStackInfo {
        ptr: rt.stack.as_ptr() as *mut Value,
        len: rt.stack.len(),
    }
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_set_stack_len(rt: *mut VmRuntime, new_len: i64) -> i64 {
    let rt = &mut *rt;
    rt.stack.truncate(new_len as usize);
    0
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_set_ip(rt: *mut VmRuntime, new_ip: i32) -> i64 {
    let rt = &mut *rt;
    if let Some(frame) = rt.frames.last_mut() {
        frame.ip = new_ip as usize;
        0
    } else {
        -1
    }
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_pop_n(rt: *mut VmRuntime, n: i32) -> i64 {
    let rt = &mut *rt;
    for _ in 0..n {
        if rt.stack.pop().is_none() {
            return jit_err(rt, VmError::StackUnderflow);
        }
    }
    0
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_push_ic_value(
    rt: *mut VmRuntime,
    ic: *const AttributeIC,
) -> i64 {
    let rt = &mut *rt;
    let ic = &*ic;
    match rt.push_value(ic.value.clone()) {
        Ok(()) => 0,
        Err(e) => jit_err(rt, e),
    }
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_push_index_ic_value(
    rt: *mut VmRuntime,
    ic: *const IndexIC,
) -> i64 {
    let rt = &mut *rt;
    let ic = &*ic;
    match rt.push_value(ic.value.clone()) {
        Ok(()) => 0,
        Err(e) => jit_err(rt, e),
    }
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_get_field_cached(
    rt: *mut VmRuntime,
    ic_ptr: *mut AttributeIC,
    field_name_ptr: *const u8,
    field_name_len: usize,
) -> i64 {
    jit_wrap(rt, |rt| {
        let field_name =
            match std::str::from_utf8(std::slice::from_raw_parts(field_name_ptr, field_name_len)) {
                Ok(s) => s,
                Err(e) => {
                    return jit_err(rt, VmError::RuntimeError(format!("Invalid UTF-8: {}", e)))
                }
            };
        let _field = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let target = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let ic = match ic_ptr.as_mut() {
            Some(ic) => ic,
            None => return jit_err(rt, VmError::RuntimeError("Null IC".to_string())),
        };

        if let Value::Pointer(ptr) = target {
            if ptr == ic.target_ptr {
                if let Some(slot) = rt.heap.slots.get(ptr).and_then(|s| s.as_ref()) {
                    if slot.generation == ic.generation && slot.version == ic.version {
                        return match rt.push_value(ic.value.clone()) {
                            Ok(()) => 0,
                            Err(e) => jit_err(rt, e),
                        };
                    }
                }
            }
            match rt.heap_get_field(ptr, field_name) {
                Ok(value) => {
                    if let Some(slot) = rt.heap.slots.get(ptr).and_then(|s| s.as_ref()) {
                        ic.target_ptr = ptr;
                        ic.generation = slot.generation;
                        ic.version = slot.version;
                        ic.value = value.clone();
                    }
                    match rt.push_value(value) {
                        Ok(()) => 0,
                        Err(e) => jit_err(rt, e),
                    }
                }
                Err(e) => jit_err(rt, e),
            }
        } else {
            match target {
                Value::Object(obj) => {
                    // Autobox for production performance.
                    match rt.heap_alloc(Value::Object(obj), 0) {
                        Ok(ptr) => match rt.heap_get_field(ptr, field_name) {
                            Ok(v) => match rt.push_value(v) {
                                Ok(()) => 0,
                                Err(e) => jit_err(rt, e),
                            },
                            Err(e) => jit_err(rt, e),
                        },
                        Err(e) => jit_err(rt, e),
                    }
                }
                _ => jit_err(
                    rt,
                    VmError::TypeError("GetField expects object".to_string()),
                ),
            }
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_get_field(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        let field = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let target = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let field = match field {
            Value::String(s) => s,
            _ => {
                return jit_err(
                    rt,
                    VmError::TypeError("field name must be string".to_string()),
                )
            }
        };
        // Reuse the interpreter behavior (including caches/autobox semantics).
        let instr = BytecodeInstr::new(OpCode::GetField, vec![], 0);
        // Push back in expected order: target, field.
        if let Err(e) = rt
            .push_value(target)
            .and_then(|_| rt.push_value(Value::String(field)))
        {
            return jit_err(rt, e);
        }
        match rt.execute_instruction(instr, 0) {
            Ok(ControlFlow::Continue) => 0,
            Ok(ControlFlow::Return(v)) => {
                rt.jit_set_retval(v);
                0
            }
            Ok(ControlFlow::Breakpoint(_)) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_set_field_cached(
    rt: *mut VmRuntime,
    ic_ptr: *mut AttributeIC,
    field_name_ptr: *const u8,
    field_name_len: usize,
) -> i64 {
    jit_wrap(rt, |rt| {
        let field_name =
            match std::str::from_utf8(std::slice::from_raw_parts(field_name_ptr, field_name_len)) {
                Ok(s) => s,
                Err(e) => {
                    return jit_err(rt, VmError::RuntimeError(format!("Invalid UTF-8: {}", e)))
                }
            };
        let value = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let _field = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let target = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let ic = match ic_ptr.as_mut() {
            Some(ic) => ic,
            None => return jit_err(rt, VmError::RuntimeError("Null IC".to_string())),
        };

        if let Value::Pointer(ptr) = target {
            match rt.heap_set_field(ptr, field_name, value.clone()) {
                Ok(()) => {
                    if let Some(slot) = rt.heap.slots.get(ptr).and_then(|s| s.as_ref()) {
                        ic.target_ptr = ptr;
                        ic.generation = slot.generation;
                        ic.version = slot.version;
                        ic.value = value;
                    }
                    match rt.push_value(Value::Pointer(ptr)) {
                        Ok(()) => 0,
                        Err(e) => jit_err(rt, e),
                    }
                }
                Err(e) => jit_err(rt, e),
            }
        } else {
            match target {
                Value::Object(mut obj) => {
                    obj.insert(field_name.to_string(), value.clone());
                    match rt.heap_alloc(Value::Object(obj), 0) {
                        Ok(ptr) => {
                            if let Some(slot) = rt.heap.slots.get(ptr).and_then(|s| s.as_ref()) {
                                ic.target_ptr = ptr;
                                ic.generation = slot.generation;
                                ic.version = slot.version;
                                ic.value = value;
                            }
                            match rt.push_value(Value::Pointer(ptr)) {
                                Ok(()) => 0,
                                Err(e) => jit_err(rt, e),
                            }
                        }
                        Err(e) => jit_err(rt, e),
                    }
                }
                _ => jit_err(
                    rt,
                    VmError::TypeError("SetField expects object".to_string()),
                ),
            }
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_set_field(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        let instr = BytecodeInstr::new(OpCode::SetField, vec![], 0);
        match rt.execute_instruction(instr, 0) {
            Ok(ControlFlow::Continue) => 0,
            Ok(ControlFlow::Return(v)) => {
                rt.jit_set_retval(v);
                0
            }
            Ok(ControlFlow::Breakpoint(_)) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_get_index_cached(
    rt: *mut VmRuntime,
    ic_ptr: *mut IndexIC,
    index: i64,
) -> i64 {
    jit_wrap(rt, |rt| {
        let _index = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let target = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let ic = match ic_ptr.as_mut() {
            Some(ic) => ic,
            None => return jit_err(rt, VmError::RuntimeError("Null IC".to_string())),
        };

        if let Value::Pointer(ptr) = target {
            if ptr == ic.target_ptr && index == ic.index {
                if let Some(slot) = rt.heap.slots.get(ptr).and_then(|s| s.as_ref()) {
                    if slot.generation == ic.generation && slot.version == ic.version {
                        return match rt.push_value(ic.value.clone()) {
                            Ok(()) => 0,
                            Err(e) => jit_err(rt, e),
                        };
                    }
                }
            }

            match rt.heap_get(ptr) {
                Ok(resolved) => {
                    let value = match resolved {
                        Value::Array(arr) => {
                            if index < 0 || index as usize >= arr.len() {
                                return jit_err(
                                    rt,
                                    VmError::InvalidOperand(format!(
                                        "index {} out of bounds",
                                        index
                                    )),
                                );
                            }
                            arr[index as usize].clone()
                        }
                        _ => {
                            return jit_err(
                                rt,
                                VmError::TypeError("GetIndex expects array".to_string()),
                            )
                        }
                    };

                    if let Some(slot) = rt.heap.slots.get(ptr).and_then(|s| s.as_ref()) {
                        ic.target_ptr = ptr;
                        ic.index = index;
                        ic.generation = slot.generation;
                        ic.version = slot.version;
                        ic.value = value.clone();
                    }
                    match rt.push_value(value) {
                        Ok(()) => 0,
                        Err(e) => jit_err(rt, e),
                    }
                }
                Err(e) => jit_err(rt, e),
            }
        } else {
            match target {
                Value::Array(arr) => {
                    if index < 0 || index as usize >= arr.len() {
                        return jit_err(
                            rt,
                            VmError::InvalidOperand(format!("index {} out of bounds", index)),
                        );
                    }
                    match rt.push_value(arr[index as usize].clone()) {
                        Ok(()) => 0,
                        Err(e) => jit_err(rt, e),
                    }
                }
                _ => jit_err(rt, VmError::TypeError("GetIndex expects array".to_string())),
            }
        }
    })
}
#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_get_index(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        let instr = BytecodeInstr::new(OpCode::GetIndex, vec![], 0);
        match rt.execute_instruction(instr, 0) {
            Ok(ControlFlow::Continue) => 0,
            Ok(ControlFlow::Return(v)) => {
                rt.jit_set_retval(v);
                0
            }
            Ok(ControlFlow::Breakpoint(_)) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_set_index_cached(
    rt: *mut VmRuntime,
    ic_ptr: *mut IndexIC,
    index: i64,
) -> i64 {
    jit_wrap(rt, |rt| {
        let value = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let _index = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let target = match rt.pop_value() {
            Ok(v) => v,
            Err(e) => return jit_err(rt, e),
        };
        let ic = match ic_ptr.as_mut() {
            Some(ic) => ic,
            None => return jit_err(rt, VmError::RuntimeError("Null IC".to_string())),
        };

        if let Value::Pointer(ptr) = target {
            {
                let obj = match rt.heap_get_mut(ptr) {
                    Ok(o) => o,
                    Err(e) => return jit_err(rt, e),
                };
                match obj {
                    Value::Array(arr) => {
                        if index < 0 || index as usize >= arr.len() {
                            return jit_err(
                                rt,
                                VmError::InvalidOperand(format!("index {} out of bounds", index)),
                            );
                        }
                        arr[index as usize] = value.clone();
                    }
                    _ => {
                        return jit_err(
                            rt,
                            VmError::TypeError("SetIndex expects array".to_string()),
                        )
                    }
                }
            }
            if let Some(slot) = rt.heap.slots.get(ptr).and_then(|s| s.as_ref()) {
                ic.target_ptr = ptr;
                ic.index = index;
                ic.generation = slot.generation;
                ic.version = slot.version;
                ic.value = value;
            }
            match rt.push_value(Value::Pointer(ptr)) {
                Ok(()) => 0,
                Err(e) => jit_err(rt, e),
            }
        } else {
            match target {
                Value::Array(mut arr) => {
                    if index < 0 || index as usize >= arr.len() {
                        return jit_err(
                            rt,
                            VmError::InvalidOperand(format!("index {} out of bounds", index)),
                        );
                    }
                    arr[index as usize] = value.clone();
                    match rt.heap_alloc(Value::Array(arr), 0) {
                        Ok(ptr) => {
                            if let Some(slot) = rt.heap.slots.get(ptr).and_then(|s| s.as_ref()) {
                                ic.target_ptr = ptr;
                                ic.index = index;
                                ic.generation = slot.generation;
                                ic.version = slot.version;
                                ic.value = value;
                            }
                            match rt.push_value(Value::Pointer(ptr)) {
                                Ok(()) => 0,
                                Err(e) => jit_err(rt, e),
                            }
                        }
                        Err(e) => jit_err(rt, e),
                    }
                }
                _ => jit_err(rt, VmError::TypeError("SetIndex expects array".to_string())),
            }
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_set_index(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        let instr = BytecodeInstr::new(OpCode::SetIndex, vec![], 0);
        match rt.execute_instruction(instr, 0) {
            Ok(ControlFlow::Continue) => 0,
            Ok(ControlFlow::Return(v)) => {
                rt.jit_set_retval(v);
                0
            }
            Ok(ControlFlow::Breakpoint(_)) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_len(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        let instr = BytecodeInstr::new(OpCode::LEN, vec![], 0);
        match rt.execute_instruction(instr, 0) {
            Ok(ControlFlow::Continue) => 0,
            Ok(ControlFlow::Return(v)) => {
                rt.jit_set_retval(v);
                0
            }
            Ok(ControlFlow::Breakpoint(_)) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_slice(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        let instr = BytecodeInstr::new(OpCode::SLICE, vec![], 0);
        match rt.execute_instruction(instr, 0) {
            Ok(ControlFlow::Continue) => 0,
            Ok(ControlFlow::Return(v)) => {
                rt.jit_set_retval(v);
                0
            }
            Ok(ControlFlow::Breakpoint(_)) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_get_global(rt: *mut VmRuntime, idx: i32) -> i64 {
    jit_wrap(rt, |rt| {
        let instr = BytecodeInstr::new(OpCode::GetGlobal, vec![idx], 0);
        match rt.execute_instruction(instr, 0) {
            Ok(ControlFlow::Continue) => 0,
            Ok(ControlFlow::Return(v)) => {
                rt.jit_set_retval(v);
                0
            }
            Ok(ControlFlow::Breakpoint(_)) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_set_global(rt: *mut VmRuntime, idx: i32) -> i64 {
    jit_wrap(rt, |rt| {
        let instr = BytecodeInstr::new(OpCode::SetGlobal, vec![idx], 0);
        match rt.execute_instruction(instr, 0) {
            Ok(ControlFlow::Continue) => 0,
            Ok(ControlFlow::Return(v)) => {
                rt.jit_set_retval(v);
                0
            }
            Ok(ControlFlow::Breakpoint(_)) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_get_global_m(rt: *mut VmRuntime, idx: i32) -> i64 {
    jit_wrap(rt, |rt| {
        let instr = BytecodeInstr::new(OpCode::GetGlobalM, vec![idx], 0);
        match rt.execute_instruction(instr, 0) {
            Ok(ControlFlow::Continue) => 0,
            Ok(ControlFlow::Return(v)) => {
                rt.jit_set_retval(v);
                0
            }
            Ok(ControlFlow::Breakpoint(_)) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_set_global_m(rt: *mut VmRuntime, idx: i32) -> i64 {
    jit_wrap(rt, |rt| {
        let instr = BytecodeInstr::new(OpCode::SetGlobalM, vec![idx], 0);
        match rt.execute_instruction(instr, 0) {
            Ok(ControlFlow::Continue) => 0,
            Ok(ControlFlow::Return(v)) => {
                rt.jit_set_retval(v);
                0
            }
            Ok(ControlFlow::Breakpoint(_)) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_alloc(rt: *mut VmRuntime, size_hint: i32) -> i64 {
    jit_wrap(rt, |rt| {
        let instr = BytecodeInstr::new(OpCode::ALLOC, vec![size_hint], 0);
        match rt.execute_instruction(instr, 0) {
            Ok(ControlFlow::Continue) => 0,
            Ok(ControlFlow::Return(v)) => {
                rt.jit_set_retval(v);
                0
            }
            Ok(ControlFlow::Breakpoint(_)) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_free(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        let instr = BytecodeInstr::new(OpCode::FREE, vec![], 0);
        match rt.execute_instruction(instr, 0) {
            Ok(ControlFlow::Continue) => 0,
            Ok(ControlFlow::Return(v)) => {
                rt.jit_set_retval(v);
                0
            }
            Ok(ControlFlow::Breakpoint(_)) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_ret(rt: *mut VmRuntime) -> i64 {
    jit_wrap(rt, |rt| {
        let Some(frame) = rt.frames.last() else {
            return jit_err(rt, VmError::RuntimeError("No frames".to_string()));
        };
        let value = if rt.stack.len() > frame.stack_base {
            match rt.pop_value() {
                Ok(v) => v,
                Err(e) => return jit_err(rt, e),
            }
        } else {
            Value::Unit
        };
        rt.jit_set_retval(value);
        0
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_call(
    rt: *mut VmRuntime,
    func_raw: i32,
    num_args: i32,
) -> i64 {
    jit_wrap(rt, |rt| {
        let instr = BytecodeInstr::new(OpCode::CALL, vec![func_raw, num_args], 0);
        match rt.execute_instruction(instr, 0) {
            Ok(ControlFlow::Continue) => 0,
            Ok(ControlFlow::Return(v)) => {
                rt.jit_set_retval(v);
                0
            }
            Ok(ControlFlow::Breakpoint(_)) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_call_ext(
    rt: *mut VmRuntime,
    name_idx: i32,
    num_args: i32,
) -> i64 {
    jit_wrap(rt, |rt| {
        let instr = BytecodeInstr::new(OpCode::CallExt, vec![name_idx, num_args], 0);
        match rt.execute_instruction(instr, 0) {
            Ok(ControlFlow::Continue) => 0,
            Ok(ControlFlow::Return(v)) => {
                rt.jit_set_retval(v);
                0
            }
            Ok(ControlFlow::Breakpoint(_)) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_closure(
    rt: *mut VmRuntime,
    func_idx: i32,
    num_upvalues: i32,
) -> i64 {
    jit_wrap(rt, |rt| {
        let instr = BytecodeInstr::new(OpCode::CLOSURE, vec![func_idx, num_upvalues], 0);
        match rt.execute_instruction(instr, 0) {
            Ok(ControlFlow::Continue) => 0,
            Ok(ControlFlow::Return(v)) => {
                rt.jit_set_retval(v);
                0
            }
            Ok(ControlFlow::Breakpoint(_)) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_closure_ref(
    rt: *mut VmRuntime,
    func_idx: i32,
    num_upvalues: i32,
    indices: *const i32,
) -> i64 {
    jit_wrap(rt, |rt| {
        let mut operands = vec![func_idx, num_upvalues];
        let idx_slice = std::slice::from_raw_parts(indices, num_upvalues as usize);
        operands.extend_from_slice(idx_slice);
        let instr = BytecodeInstr::new(OpCode::ClosureRef, operands, 0);
        match rt.execute_instruction(instr, 0) {
            Ok(ControlFlow::Continue) => 0,
            Ok(ControlFlow::Return(v)) => {
                rt.jit_set_retval(v);
                0
            }
            Ok(ControlFlow::Breakpoint(_)) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn nyx_jit_closure_ref_stack(
    rt: *mut VmRuntime,
    func_idx: i32,
    num_upvalues: i32,
) -> i64 {
    jit_wrap(rt, |rt| {
        let instr = BytecodeInstr::new(OpCode::ClosureRefStack, vec![func_idx, num_upvalues], 0);
        match rt.execute_instruction(instr, 0) {
            Ok(ControlFlow::Continue) => 0,
            Ok(ControlFlow::Return(v)) => {
                rt.jit_set_retval(v);
                0
            }
            Ok(ControlFlow::Breakpoint(_)) => 0,
            Err(e) => jit_err(rt, e),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::{BytecodeInstr, BytecodeModule, Function, OpCode, Value};

    #[test]
    fn test_vm_new() {
        let vm = NyxVm::new(VmConfig::default());
        assert_eq!(vm.runtime.instruction_count(), 0);
    }

    #[test]
    fn test_vm_runtime_new() {
        let runtime = VmRuntime::new(VmConfig::default());
        assert!(runtime.stack.is_empty());
        assert!(runtime.frames.is_empty());
    }

    #[test]
    fn test_vm_call_and_return() {
        let mut module = BytecodeModule::new("main".to_string());
        let c0 = 0;
        let c1 = 1;

        let add_fn = Function {
            name: "add".to_string(),
            arity: 2,
            num_locals: 2,
            instructions: vec![
                BytecodeInstr::with_operand(OpCode::LOAD, 0, 0),
                BytecodeInstr::with_operand(OpCode::LOAD, 1, 0),
                BytecodeInstr::new(OpCode::ADD, vec![], 0),
                BytecodeInstr::new(OpCode::RET, vec![], 0),
            ],
            constants: vec![],
            upvalues: vec![],
            line_info: vec![],
        };
        let add_idx = module.add_function(add_fn);

        let main_fn = Function {
            name: "main".to_string(),
            arity: 0,
            num_locals: 0,
            instructions: vec![
                BytecodeInstr::with_operand(OpCode::PUSH, c0, 0),
                BytecodeInstr::with_operand(OpCode::PUSH, c1, 0),
                BytecodeInstr::new(OpCode::CALL, vec![add_idx as i32, 2], 0),
                BytecodeInstr::new(OpCode::RET, vec![], 0),
            ],
            constants: vec![Value::Int(2), Value::Int(3)],
            upvalues: vec![],
            line_info: vec![],
        };
        module.add_function(main_fn);

        let mut vm = NyxVm::new(VmConfig::default());
        vm.load(module);
        let result = vm.run("main").unwrap();
        assert_eq!(result, Value::Int(5));
    }

    #[test]
    fn test_vm_instruction_limit() {
        let mut module = BytecodeModule::new("main".to_string());
        let main_fn = Function {
            name: "main".to_string(),
            arity: 0,
            num_locals: 0,
            instructions: vec![BytecodeInstr::with_operand(OpCode::JMP, 0, 0)],
            constants: vec![],
            upvalues: vec![],
            line_info: vec![],
        };
        module.add_function(main_fn);

        let config = VmConfig {
            max_instructions: 16,
            ..Default::default()
        };
        let mut vm = NyxVm::new(config);
        vm.load(module);
        let result = vm.run("main");
        assert!(matches!(result, Err(VmError::InstructionLimitExceeded(16))));
    }

    #[test]
    fn test_vm_native_call() {
        let mut module = BytecodeModule::new("main".to_string());
        let main_fn = Function {
            name: "main".to_string(),
            arity: 0,
            num_locals: 0,
            instructions: vec![
                BytecodeInstr::with_operand(OpCode::PUSH, 1, 0),
                BytecodeInstr::with_operand(OpCode::PUSH, 2, 0),
                BytecodeInstr::new(OpCode::CallExt, vec![0, 2], 0),
                BytecodeInstr::new(OpCode::RET, vec![], 0),
            ],
            constants: vec![
                Value::String("sum".to_string()),
                Value::Int(2),
                Value::Int(3),
            ],
            upvalues: vec![],
            line_info: vec![],
        };
        module.add_function(main_fn);

        let mut vm = NyxVm::new(VmConfig::default());
        vm.register("sum", 2, |args| {
            if let (Value::Int(a), Value::Int(b)) = (&args[0], &args[1]) {
                Ok(Value::Int(a + b))
            } else {
                Err("expected ints".to_string())
            }
        });
        vm.load(module);
        let result = vm.run("main").unwrap();
        assert_eq!(result, Value::Int(5));
    }

    #[test]
    fn test_vm_heap_gc() {
        let mut module = BytecodeModule::new("main".to_string());
        let main_fn = Function {
            name: "main".to_string(),
            arity: 0,
            num_locals: 0,
            instructions: vec![
                BytecodeInstr::with_operand(OpCode::PUSH, 0, 0),
                BytecodeInstr::new(OpCode::ALLOC, vec![], 0),
                BytecodeInstr::new(OpCode::POP, vec![], 0),
                BytecodeInstr::with_operand(OpCode::PUSH, 0, 0),
                BytecodeInstr::new(OpCode::ALLOC, vec![], 0),
                BytecodeInstr::new(OpCode::POP, vec![], 0),
                BytecodeInstr::with_operand(OpCode::PUSH, 0, 0),
                BytecodeInstr::new(OpCode::ALLOC, vec![], 0),
                BytecodeInstr::new(OpCode::POP, vec![], 0),
                BytecodeInstr::new(OpCode::RET, vec![], 0),
            ],
            constants: vec![Value::Int(1)],
            upvalues: vec![],
            line_info: vec![],
        };
        module.add_function(main_fn);

        let config = VmConfig {
            heap_size: std::mem::size_of::<Value>(),
            enable_gc: true,
            ..Default::default()
        };
        let mut vm = NyxVm::new(config);
        vm.load(module);
        let result = vm.run("main");
        assert!(result.is_ok());
    }

    #[test]
    fn test_vm_negative_operand_rejected() {
        let mut module = BytecodeModule::new("main".to_string());
        let main_fn = Function {
            name: "main".to_string(),
            arity: 0,
            num_locals: 0,
            instructions: vec![
                BytecodeInstr::with_operand(OpCode::LOAD, -1, 0),
                BytecodeInstr::new(OpCode::RET, vec![], 0),
            ],
            constants: vec![],
            upvalues: vec![],
            line_info: vec![],
        };
        module.add_function(main_fn);

        let mut vm = NyxVm::new(VmConfig::default());
        vm.load(module);
        let result = vm.run("main");
        assert!(matches!(result, Err(VmError::InvalidOperand(_))));
    }

    #[test]
    fn test_vm_push_module_constant() {
        let mut module = BytecodeModule::new("main".to_string());
        module.constants.push(Value::Int(42));
        let main_fn = Function {
            name: "main".to_string(),
            arity: 0,
            num_locals: 0,
            instructions: vec![
                BytecodeInstr::with_operand(OpCode::PushM, 0, 0),
                BytecodeInstr::new(OpCode::RET, vec![], 0),
            ],
            constants: vec![],
            upvalues: vec![],
            line_info: vec![],
        };
        module.add_function(main_fn);

        let mut vm = NyxVm::new(VmConfig::default());
        vm.load(module);
        let result = vm.run("main").unwrap();
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_vm_get_set_global_module() {
        let mut module = BytecodeModule::new("main".to_string());
        module.constants.push(Value::String("g".to_string()));
        let main_fn = Function {
            name: "main".to_string(),
            arity: 0,
            num_locals: 0,
            instructions: vec![
                BytecodeInstr::with_operand(OpCode::PUSH, 0, 0),
                BytecodeInstr::with_operand(OpCode::SetGlobalM, 0, 0),
                BytecodeInstr::with_operand(OpCode::GetGlobalM, 0, 0),
                BytecodeInstr::new(OpCode::RET, vec![], 0),
            ],
            constants: vec![Value::Int(9)],
            upvalues: vec![],
            line_info: vec![],
        };
        module.add_function(main_fn);

        let mut vm = NyxVm::new(VmConfig::default());
        vm.load(module);
        let result = vm.run("main").unwrap();
        assert_eq!(result, Value::Int(9));
    }

    #[test]
    fn test_vm_loop_soak_smoke() {
        let mut module = BytecodeModule::new("main".to_string());
        let loop_start = 4;
        let loop_end = 13;
        let main_fn = Function {
            name: "main".to_string(),
            arity: 0,
            num_locals: 2,
            instructions: vec![
                BytecodeInstr::with_operand(OpCode::PUSH, 0, 0),  // 0
                BytecodeInstr::with_operand(OpCode::STORE, 0, 0), // counter
                BytecodeInstr::with_operand(OpCode::PUSH, 1, 0),  // limit
                BytecodeInstr::with_operand(OpCode::STORE, 1, 0), // limit local
                BytecodeInstr::with_operand(OpCode::LOAD, 0, 0),  // loop_start
                BytecodeInstr::with_operand(OpCode::LOAD, 1, 0),
                BytecodeInstr::new(OpCode::LT, vec![], 0),
                BytecodeInstr::with_operand(OpCode::JZ, loop_end, 0),
                BytecodeInstr::with_operand(OpCode::LOAD, 0, 0),
                BytecodeInstr::with_operand(OpCode::PUSH, 2, 0),
                BytecodeInstr::new(OpCode::ADD, vec![], 0),
                BytecodeInstr::with_operand(OpCode::STORE, 0, 0),
                BytecodeInstr::with_operand(OpCode::JMP, loop_start, 0),
                BytecodeInstr::with_operand(OpCode::LOAD, 0, 0), // loop_end
                BytecodeInstr::new(OpCode::RET, vec![], 0),
            ],
            constants: vec![Value::Int(0), Value::Int(10_000), Value::Int(1)],
            upvalues: vec![],
            line_info: vec![],
        };
        module.add_function(main_fn);

        let mut vm = NyxVm::new(VmConfig::default());
        vm.load(module);
        let result = vm.run("main").unwrap();
        assert_eq!(result, Value::Int(10_000));
    }

    #[test]
    fn test_vm_closure_call() {
        let mut module = BytecodeModule::new("main".to_string());
        let add_fn = Function {
            name: "adder".to_string(),
            arity: 1,
            num_locals: 2,
            instructions: vec![
                BytecodeInstr::with_operand(OpCode::LOAD, 0, 0),
                BytecodeInstr::with_operand(OpCode::LOAD, 1, 0),
                BytecodeInstr::new(OpCode::ADD, vec![], 0),
                BytecodeInstr::new(OpCode::RET, vec![], 0),
            ],
            constants: vec![],
            upvalues: vec!["x".to_string()],
            line_info: vec![],
        };
        let add_idx = module.add_function(add_fn);

        let main_fn = Function {
            name: "main".to_string(),
            arity: 0,
            num_locals: 0,
            instructions: vec![
                BytecodeInstr::with_operand(OpCode::PUSH, 0, 0),
                BytecodeInstr::new(OpCode::CLOSURE, vec![add_idx as i32, 1], 0),
                BytecodeInstr::with_operand(OpCode::PUSH, 1, 0),
                BytecodeInstr::new(OpCode::SWAP, vec![], 0),
                BytecodeInstr::new(OpCode::CALL, vec![-1, 1], 0),
                BytecodeInstr::new(OpCode::RET, vec![], 0),
            ],
            constants: vec![Value::Int(3), Value::Int(4)],
            upvalues: vec![],
            line_info: vec![],
        };
        module.add_function(main_fn);

        let mut vm = NyxVm::new(VmConfig::default());
        vm.load(module);
        let result = vm.run("main").unwrap();
        assert_eq!(result, Value::Int(7));
    }

    #[test]
    fn test_vm_closure_ref_call() {
        let mut module = BytecodeModule::new("main".to_string());
        let add_fn = Function {
            name: "adder_ref".to_string(),
            arity: 1,
            num_locals: 2,
            instructions: vec![
                BytecodeInstr::with_operand(OpCode::LOAD, 0, 0),
                BytecodeInstr::with_operand(OpCode::LOAD, 1, 0),
                BytecodeInstr::new(OpCode::ADD, vec![], 0),
                BytecodeInstr::new(OpCode::RET, vec![], 0),
            ],
            constants: vec![],
            upvalues: vec!["x".to_string()],
            line_info: vec![],
        };
        let add_idx = module.add_function(add_fn);

        let main_fn = Function {
            name: "main".to_string(),
            arity: 0,
            num_locals: 1,
            instructions: vec![
                BytecodeInstr::with_operand(OpCode::PUSH, 0, 0),
                BytecodeInstr::with_operand(OpCode::STORE, 0, 0),
                BytecodeInstr::new(OpCode::ClosureRef, vec![add_idx as i32, 1, 0], 0),
                BytecodeInstr::with_operand(OpCode::PUSH, 1, 0),
                BytecodeInstr::with_operand(OpCode::STORE, 0, 0),
                BytecodeInstr::with_operand(OpCode::PUSH, 2, 0),
                BytecodeInstr::new(OpCode::SWAP, vec![], 0),
                BytecodeInstr::new(OpCode::CALL, vec![-1, 1], 0),
                BytecodeInstr::new(OpCode::RET, vec![], 0),
            ],
            constants: vec![Value::Int(1), Value::Int(5), Value::Int(2)],
            upvalues: vec![],
            line_info: vec![],
        };
        module.add_function(main_fn);

        let mut vm = NyxVm::new(VmConfig::default());
        vm.load(module);
        let result = vm.run("main").unwrap();
        assert_eq!(result, Value::Int(7));
    }

    #[test]
    fn test_vm_closure_ref_stack_call() {
        let mut module = BytecodeModule::new("main".to_string());
        let add_fn = Function {
            name: "adder_ref_stack".to_string(),
            arity: 1,
            num_locals: 2,
            instructions: vec![
                BytecodeInstr::with_operand(OpCode::LOAD, 0, 0),
                BytecodeInstr::with_operand(OpCode::LOAD, 1, 0),
                BytecodeInstr::new(OpCode::ADD, vec![], 0),
                BytecodeInstr::new(OpCode::RET, vec![], 0),
            ],
            constants: vec![],
            upvalues: vec!["x".to_string()],
            line_info: vec![],
        };
        let add_idx = module.add_function(add_fn);

        let main_fn = Function {
            name: "main".to_string(),
            arity: 0,
            num_locals: 0,
            instructions: vec![
                BytecodeInstr::with_operand(OpCode::PUSH, 0, 0),
                BytecodeInstr::new(OpCode::ClosureRefStack, vec![add_idx as i32, 1], 0),
                BytecodeInstr::with_operand(OpCode::PUSH, 1, 0),
                BytecodeInstr::new(OpCode::SWAP, vec![], 0),
                BytecodeInstr::new(OpCode::CALL, vec![-1, 1], 0),
                BytecodeInstr::new(OpCode::RET, vec![], 0),
            ],
            constants: vec![Value::Int(10), Value::Int(5)],
            upvalues: vec![],
            line_info: vec![],
        };
        module.add_function(main_fn);

        let mut vm = NyxVm::new(VmConfig::default());
        vm.load(module);
        let result = vm.run("main").unwrap();
        assert_eq!(result, Value::Int(15));
    }
}

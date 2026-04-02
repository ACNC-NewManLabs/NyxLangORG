use nyx_vm::{BytecodeModule, Function, Instruction, OpCode, Value, NyxVm, VmConfig};

#[test]
fn test_jit_ic_get_field() {
    let mut module = BytecodeModule::new("main".to_string());
    
    // Create an object { "a": 42 }, then access "a" in a loop.
    let mut main_instrs = Vec::new();
    main_instrs.push(Instruction::with_operand(OpCode::PUSH, 0, 0)); // "a"
    main_instrs.push(Instruction::with_operand(OpCode::PUSH, 1, 0)); // 42
    main_instrs.push(Instruction::with_operand(OpCode::NewObj, 1, 0));
    
    // Loop 100 times accessing "a"
    for _ in 0..100 {
        main_instrs.push(Instruction::new(OpCode::DUP, vec![], 0));
        main_instrs.push(Instruction::with_operand(OpCode::PUSH, 0, 0)); // "a"
        main_instrs.push(Instruction::new(OpCode::GetField, vec![], 0));
        main_instrs.push(Instruction::new(OpCode::POP, vec![], 0));
    }
    
    main_instrs.push(Instruction::with_operand(OpCode::PUSH, 0, 0)); // "a"
    main_instrs.push(Instruction::new(OpCode::GetField, vec![], 0));
    main_instrs.push(Instruction::new(OpCode::RET, vec![], 0));
    
    let main_fn = Function {
        name: "main".to_string(),
        arity: 0,
        num_locals: 0,
        instructions: main_instrs,
        constants: vec![Value::String("a".to_string()), Value::Int(42)],
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
fn test_jit_high_arity() {
    let mut module = BytecodeModule::new("main".to_string());
    
    // Function with 8 arguments: returns sum of all.
    let mut instrs = Vec::new();
    for i in 0..8 {
        instrs.push(Instruction::with_operand(OpCode::LOAD, i as i32, 0));
    }
    for _ in 0..7 {
        instrs.push(Instruction::new(OpCode::ADD, vec![], 0));
    }
    instrs.push(Instruction::new(OpCode::RET, vec![], 0));

    let sum_fn = Function {
        name: "sum8".to_string(),
        arity: 8,
        num_locals: 8,
        instructions: instrs,
        constants: vec![],
        upvalues: vec![],
        line_info: vec![],
    };
    let sum_idx = module.add_function(sum_fn);

    let main_fn = Function {
        name: "main".to_string(),
        arity: 0,
        num_locals: 0,
        instructions: vec![
            Instruction::with_operand(OpCode::PUSH, 0, 0),
            Instruction::with_operand(OpCode::PUSH, 1, 0),
            Instruction::with_operand(OpCode::PUSH, 2, 0),
            Instruction::with_operand(OpCode::PUSH, 3, 0),
            Instruction::with_operand(OpCode::PUSH, 4, 0),
            Instruction::with_operand(OpCode::PUSH, 5, 0),
            Instruction::with_operand(OpCode::PUSH, 6, 0),
            Instruction::with_operand(OpCode::PUSH, 7, 0),
            Instruction::new(OpCode::CALL, vec![sum_idx as i32, 8], 0),
            Instruction::new(OpCode::RET, vec![], 0),
        ],
        constants: vec![
            Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4),
            Value::Int(5), Value::Int(6), Value::Int(7), Value::Int(8),
        ],
        upvalues: vec![],
        line_info: vec![],
    };
    module.add_function(main_fn);

    let mut config = VmConfig::default();
    config.enable_jit = true;
    let mut vm = NyxVm::new(config);
    vm.load(module);
    let result = vm.run("main").unwrap();
    // 1+2+3+4+5+6+7+8 = 36
    assert_eq!(result, Value::Int(36));
}

#[test]
fn test_vm_jit_with_closures() {
    let mut module = BytecodeModule::new("main".to_string());
    
    // Inner function: captures 'x' (local 0 in outer) and adds 'y' (arg 0).
    let inner_fn = Function {
        name: "inner".to_string(),
        arity: 1,
        num_locals: 2,
        instructions: vec![
            Instruction::with_operand(OpCode::LOAD, 0, 0), // captured x
            Instruction::with_operand(OpCode::LOAD, 1, 0), // arg y
            Instruction::new(OpCode::ADD, vec![], 0),
            Instruction::new(OpCode::RET, vec![], 0),
        ],
        constants: vec![],
        upvalues: vec!["x".to_string()],
        line_info: vec![],
    };
    let inner_idx = module.add_function(inner_fn);

    // Outer function: creates closure and calls it.
    let outer_fn = Function {
        name: "outer".to_string(),
        arity: 1, // takes x
        num_locals: 1,
        instructions: vec![
            Instruction::with_operand(OpCode::LOAD, 0, 0),
            Instruction::new(OpCode::CLOSURE, vec![inner_idx as i32, 1], 0),
            Instruction::with_operand(OpCode::PUSH, 0, 0), // arg y = 10
            Instruction::new(OpCode::SWAP, vec![], 0),
            Instruction::new(OpCode::CALL, vec![-1, 1], 0),
            Instruction::new(OpCode::RET, vec![], 0),
        ],
        constants: vec![Value::Int(10)],
        upvalues: vec![],
        line_info: vec![],
    };
    let outer_idx = module.add_function(outer_fn);

    let main_fn = Function {
        name: "main".to_string(),
        arity: 0,
        num_locals: 0,
        instructions: vec![
            Instruction::with_operand(OpCode::PUSH, 0, 0), // arg x = 5
            Instruction::new(OpCode::CALL, vec![outer_idx as i32, 1], 0),
            Instruction::new(OpCode::RET, vec![], 0),
        ],
        constants: vec![Value::Int(5)],
        upvalues: vec![],
        line_info: vec![],
    };
    module.add_function(main_fn);

    let config = VmConfig { enable_jit: true, ..Default::default() };
    let mut vm = NyxVm::new(config);
    vm.load(module);
    let result = vm.run("main").unwrap();
    // outer(5) -> inner(10) where inner captures 5 -> 5+10 = 15
    assert_eq!(result, Value::Int(15));
}

#[test]
fn test_vm_jit_mid_entry() {
    let mut module = BytecodeModule::new("main".to_string());
    
    // Function that starts with some interpreter-only ops (e.g. print/dbg?) 
    // but then enters JIT. Actually any supported opcode will trigger JIT.
    // We'll use a loop to force repeated entry if it drops out, 
    // but here we want to test entry from ip > 0.
    
    let main_fn = Function {
        name: "main".to_string(),
        arity: 0,
        num_locals: 1,
        instructions: vec![
            Instruction::with_operand(OpCode::PUSH, 0, 0), // 0
            Instruction::with_operand(OpCode::STORE, 0, 0), // 1: x = 0
            Instruction::with_operand(OpCode::LOAD, 0, 0),  // 2
            Instruction::with_operand(OpCode::PUSH, 1, 0),  // 3
            Instruction::new(OpCode::ADD, vec![], 0),       // 4
            Instruction::new(OpCode::RET, vec![], 0),       // 5
        ],
        constants: vec![Value::Int(10), Value::Int(5)],
        upvalues: vec![],
        line_info: vec![],
    };
    module.add_function(main_fn);

    let config = VmConfig { enable_jit: true, ..Default::default() };
    // Force JIT entry after 2 instructions.
    // We can't easily force it, but if we run with JIT enabled, it will try to enter at every step now.
    let mut vm = NyxVm::new(config);
    vm.load(module);
    
    let result = vm.run("main").unwrap();
    assert_eq!(result, Value::Int(15));
}

#[test]
fn test_jit_arity_16() {
    let mut module = BytecodeModule::new("main".to_string());
    
    let mut instrs = Vec::new();
    for i in 0..16 {
        instrs.push(Instruction::with_operand(OpCode::LOAD, i, 0));
    }
    for _ in 0..(16-1) {
        instrs.push(Instruction::new(OpCode::ADD, vec![], 0));
    }
    instrs.push(Instruction::new(OpCode::RET, vec![], 0));

    let sum_fn = Function {
        name: "sum16".to_string(),
        arity: 16,
        num_locals: 16,
        instructions: instrs,
        constants: vec![],
        upvalues: vec![],
        line_info: vec![],
    };
    let sum_idx = module.add_function(sum_fn);

    let main_fn = Function {
        name: "main".to_string(),
        arity: 0,
        num_locals: 0,
        instructions: vec![
            Instruction::with_operand(OpCode::PUSH, 0, 0),
            Instruction::with_operand(OpCode::PUSH, 1, 0),
            Instruction::with_operand(OpCode::PUSH, 2, 0),
            Instruction::with_operand(OpCode::PUSH, 3, 0),
            Instruction::with_operand(OpCode::PUSH, 4, 0),
            Instruction::with_operand(OpCode::PUSH, 5, 0),
            Instruction::with_operand(OpCode::PUSH, 6, 0),
            Instruction::with_operand(OpCode::PUSH, 7, 0),
            Instruction::with_operand(OpCode::PUSH, 8, 0),
            Instruction::with_operand(OpCode::PUSH, 9, 0),
            Instruction::with_operand(OpCode::PUSH, 10, 0),
            Instruction::with_operand(OpCode::PUSH, 11, 0),
            Instruction::with_operand(OpCode::PUSH, 12, 0),
            Instruction::with_operand(OpCode::PUSH, 13, 0),
            Instruction::with_operand(OpCode::PUSH, 14, 0),
            Instruction::with_operand(OpCode::PUSH, 15, 0),
            Instruction::new(OpCode::CALL, vec![sum_idx as i32, 16], 0),
            Instruction::new(OpCode::RET, vec![], 0),
        ],
        constants: vec![Value::Int(0), Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4), Value::Int(5), Value::Int(6), Value::Int(7), Value::Int(8), Value::Int(9), Value::Int(10), Value::Int(11), Value::Int(12), Value::Int(13), Value::Int(14), Value::Int(15)],
        upvalues: vec![],
        line_info: vec![],
    };
    module.add_function(main_fn);

    let config = VmConfig { enable_jit: true, ..Default::default() };
    let mut vm = NyxVm::new(config);
    vm.load(module);
    let result = vm.run("main").unwrap();
    let expected = (0..16).sum::<i64>();
    assert_eq!(result, Value::Int(expected));
}


#[test]
fn test_jit_arity_32() {
    let mut module = BytecodeModule::new("main".to_string());
    
    let mut instrs = Vec::new();
    for i in 0..32 {
        instrs.push(Instruction::with_operand(OpCode::LOAD, i, 0));
    }
    for _ in 0..(32-1) {
        instrs.push(Instruction::new(OpCode::ADD, vec![], 0));
    }
    instrs.push(Instruction::new(OpCode::RET, vec![], 0));

    let sum_fn = Function {
        name: "sum32".to_string(),
        arity: 32,
        num_locals: 32,
        instructions: instrs,
        constants: vec![],
        upvalues: vec![],
        line_info: vec![],
    };
    let sum_idx = module.add_function(sum_fn);

    let main_fn = Function {
        name: "main".to_string(),
        arity: 0,
        num_locals: 0,
        instructions: vec![
            Instruction::with_operand(OpCode::PUSH, 0, 0),
            Instruction::with_operand(OpCode::PUSH, 1, 0),
            Instruction::with_operand(OpCode::PUSH, 2, 0),
            Instruction::with_operand(OpCode::PUSH, 3, 0),
            Instruction::with_operand(OpCode::PUSH, 4, 0),
            Instruction::with_operand(OpCode::PUSH, 5, 0),
            Instruction::with_operand(OpCode::PUSH, 6, 0),
            Instruction::with_operand(OpCode::PUSH, 7, 0),
            Instruction::with_operand(OpCode::PUSH, 8, 0),
            Instruction::with_operand(OpCode::PUSH, 9, 0),
            Instruction::with_operand(OpCode::PUSH, 10, 0),
            Instruction::with_operand(OpCode::PUSH, 11, 0),
            Instruction::with_operand(OpCode::PUSH, 12, 0),
            Instruction::with_operand(OpCode::PUSH, 13, 0),
            Instruction::with_operand(OpCode::PUSH, 14, 0),
            Instruction::with_operand(OpCode::PUSH, 15, 0),
            Instruction::with_operand(OpCode::PUSH, 16, 0),
            Instruction::with_operand(OpCode::PUSH, 17, 0),
            Instruction::with_operand(OpCode::PUSH, 18, 0),
            Instruction::with_operand(OpCode::PUSH, 19, 0),
            Instruction::with_operand(OpCode::PUSH, 20, 0),
            Instruction::with_operand(OpCode::PUSH, 21, 0),
            Instruction::with_operand(OpCode::PUSH, 22, 0),
            Instruction::with_operand(OpCode::PUSH, 23, 0),
            Instruction::with_operand(OpCode::PUSH, 24, 0),
            Instruction::with_operand(OpCode::PUSH, 25, 0),
            Instruction::with_operand(OpCode::PUSH, 26, 0),
            Instruction::with_operand(OpCode::PUSH, 27, 0),
            Instruction::with_operand(OpCode::PUSH, 28, 0),
            Instruction::with_operand(OpCode::PUSH, 29, 0),
            Instruction::with_operand(OpCode::PUSH, 30, 0),
            Instruction::with_operand(OpCode::PUSH, 31, 0),
            Instruction::new(OpCode::CALL, vec![sum_idx as i32, 32], 0),
            Instruction::new(OpCode::RET, vec![], 0),
        ],
        constants: vec![Value::Int(0), Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4), Value::Int(5), Value::Int(6), Value::Int(7), Value::Int(8), Value::Int(9), Value::Int(10), Value::Int(11), Value::Int(12), Value::Int(13), Value::Int(14), Value::Int(15), Value::Int(16), Value::Int(17), Value::Int(18), Value::Int(19), Value::Int(20), Value::Int(21), Value::Int(22), Value::Int(23), Value::Int(24), Value::Int(25), Value::Int(26), Value::Int(27), Value::Int(28), Value::Int(29), Value::Int(30), Value::Int(31)],
        upvalues: vec![],
        line_info: vec![],
    };
    module.add_function(main_fn);

    let config = VmConfig { enable_jit: true, ..Default::default() };
    let mut vm = NyxVm::new(config);
    vm.load(module);
    let result = vm.run("main").unwrap();
    let expected = (0..32).sum::<i64>();
    assert_eq!(result, Value::Int(expected));
}

#[test]
fn test_jit_hardening_mutation() {
    let mut module = BytecodeModule::new("main".to_string());
    
    // SetField test: by-value object -> autobox -> IC hit
    let mut main_instrs = Vec::new();
    main_instrs.push(Instruction::with_operand(OpCode::PUSH, 0, 0)); // "a"
    main_instrs.push(Instruction::with_operand(OpCode::PUSH, 1, 0)); // 1
    main_instrs.push(Instruction::with_operand(OpCode::NewObj, 1, 0)); // Value::Object
    
    // Mutate it: should autobox to Value::Pointer
    main_instrs.push(Instruction::new(OpCode::DUP, vec![], 0));
    main_instrs.push(Instruction::with_operand(OpCode::PUSH, 0, 0)); // "a"
    main_instrs.push(Instruction::with_operand(OpCode::PUSH, 2, 0)); // 2
    main_instrs.push(Instruction::new(OpCode::SetField, vec![], 0)); // SetField returns Target (Pointer)
    
    // Check if it's now a pointer and has correct value
    main_instrs.push(Instruction::new(OpCode::DUP, vec![], 0));
    main_instrs.push(Instruction::with_operand(OpCode::PUSH, 0, 0)); // "a"
    main_instrs.push(Instruction::new(OpCode::GetField, vec![], 0)); // Should be 2
    
    // SetIndex test: by-value array -> autobox -> IC hit
    main_instrs.push(Instruction::with_operand(OpCode::PUSH, 3, 0)); // 10
    main_instrs.push(Instruction::with_operand(OpCode::NewArray, 1, 0)); // Value::Array
    
    // Mutate it
    main_instrs.push(Instruction::new(OpCode::DUP, vec![], 0));
    main_instrs.push(Instruction::with_operand(OpCode::PUSH, 4, 0)); // index 0
    main_instrs.push(Instruction::with_operand(OpCode::PUSH, 5, 0)); // 20
    main_instrs.push(Instruction::new(OpCode::SetIndex, vec![], 0)); // SetIndex returns Target (Pointer)
    
    main_instrs.push(Instruction::new(OpCode::DUP, vec![], 0));
    main_instrs.push(Instruction::with_operand(OpCode::PUSH, 4, 0)); // index 0
    main_instrs.push(Instruction::new(OpCode::GetIndex, vec![], 0)); // Should be 20
    
    main_instrs.push(Instruction::new(OpCode::RET, vec![], 0));
    
    let main_fn = Function {
        name: "main".to_string(),
        arity: 0,
        num_locals: 0,
        instructions: main_instrs,
        constants: vec![
            Value::String("a".to_string()), 
            Value::Int(1), 
            Value::Int(2),
            Value::Int(10),
            Value::Int(0),
            Value::Int(20)
        ],
        upvalues: vec![],
        line_info: vec![],
    };
    module.add_function(main_fn);
    
    let config = VmConfig { enable_jit: true, ..Default::default() };
    let mut vm = NyxVm::new(config);
    vm.load(module);
    let result = vm.run("main").unwrap();
    assert_eq!(result, Value::Int(20));
}

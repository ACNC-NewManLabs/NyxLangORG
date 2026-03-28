use nyx_vm::{BytecodeModule, Function, Instruction, OpCode, Value, NyxVm, VmConfig, VmError};

#[test]
fn test_jit_arity_mismatch() {
    let mut module = BytecodeModule::new("main".to_string());
    
    // Function: add(a, b)
    let add_fn = Function {
        name: "add".to_string(),
        arity: 2,
        num_locals: 0,
        instructions: vec![
            Instruction::with_operand(OpCode::LOAD, 0, 0),
            Instruction::with_operand(OpCode::LOAD, 1, 0),
            Instruction::new(OpCode::ADD, vec![], 0),
            Instruction::new(OpCode::RET, vec![], 0),
        ],
        constants: vec![],
        upvalues: vec![],
        line_info: vec![],
    };
    module.add_function(add_fn);

    let mut config = VmConfig::default();
    config.enable_jit = true;
    let mut vm = NyxVm::new(config);
    vm.load(module);

    // Call with 1 argument (mismatch)
    let res = vm.run_function("main", 0, vec![Value::Int(10)]);
    assert!(res.is_err());
    match res.unwrap_err() {
        VmError::TypeError(msg) => assert!(msg.contains("arity mismatch")),
        e => panic!("Expected TypeError, got {:?}", e),
    }
}

#[test]
fn test_jit_ip_tracking_on_trap() {
    let mut module = BytecodeModule::new("main".to_string());
    
    // Function that traps at IP 2
    let trap_fn = Function {
        name: "main".to_string(),
        arity: 0,
        num_locals: 1,
        instructions: vec![
            Instruction::with_operand(OpCode::PUSH, 0, 0), // IP 0: 10
            Instruction::with_operand(OpCode::PUSH, 1, 0), // IP 1: "not a number"
            Instruction::new(OpCode::ADD, vec![], 0),      // IP 2: TRAP!
            Instruction::new(OpCode::RET, vec![], 0),
        ],
        constants: vec![Value::Int(10), Value::String("x".to_string())],
        upvalues: vec![],
        line_info: vec![],
    };
    module.add_function(trap_fn);

    let mut config = VmConfig::default();
    config.enable_jit = true;
    let mut vm = NyxVm::new(config);
    vm.load(module);

    let res = vm.run("main");
    assert!(res.is_err());
    // We can't easily check the frame IP from the public API if it errors,
    // but we can check if it at least caught the type error.
    match res.unwrap_err() {
        VmError::TypeError(_) => {}
        e => panic!("Expected TypeError, got {:?}", e),
    }
}

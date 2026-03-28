use nyx_vm::{BytecodeModule, Function, Instruction, OpCode, Value, NyxVm, VmConfig};
use std::time::Instant;

fn create_numeric_module(iterations: i64) -> BytecodeModule {
    let mut module = BytecodeModule::new("main".to_string());
    
    // Function: sum(iterations)
    let main_fn = Function {
        name: "main".to_string(),
        arity: 0,
        num_locals: 2,
        instructions: vec![
            Instruction::with_operand(OpCode::PUSH, 0, 0), // 0: i = 0
            Instruction::with_operand(OpCode::STORE, 0, 0), // 1
            Instruction::with_operand(OpCode::PUSH, 0, 0), // 2: sum = 0
            Instruction::with_operand(OpCode::STORE, 1, 0), // 3
            
            // Loop start (IP 4)
            Instruction::with_operand(OpCode::LOAD, 0, 0), // 4
            Instruction::with_operand(OpCode::PUSH, 1, 0), // 5: iterations
            Instruction::new(OpCode::GE, vec![], 0), // 6
            Instruction::with_operand(OpCode::JNZ, 17, 0), // 7: jump to end if i >= iterations (IP 17)
            
            Instruction::with_operand(OpCode::LOAD, 1, 0), // 8
            Instruction::with_operand(OpCode::LOAD, 0, 0), // 9
            Instruction::new(OpCode::ADD, vec![], 0),     // 10
            Instruction::with_operand(OpCode::STORE, 1, 0), // 11
            
            Instruction::with_operand(OpCode::LOAD, 0, 0), // 12
            Instruction::with_operand(OpCode::PUSH, 2, 0), // 13: 1
            Instruction::new(OpCode::ADD, vec![], 0),     // 14
            Instruction::with_operand(OpCode::STORE, 0, 0), // 15
            
            Instruction::with_operand(OpCode::JMP, 4, 0), // 16: Loop back
            
            Instruction::with_operand(OpCode::LOAD, 1, 0), // 17
            Instruction::new(OpCode::RET, vec![], 0),      // 18
        ],
        constants: vec![Value::Int(0), Value::Int(iterations), Value::Int(1)],
        upvalues: vec![],
        line_info: vec![],
    };
    module.add_function(main_fn);
    module
}

fn create_ic_module(iterations: i64) -> BytecodeModule {
    let mut module = BytecodeModule::new("main".to_string());
    
    // Function: ic_test(iterations)
    let main_fn = Function {
        name: "main".to_string(),
        arity: 0,
        num_locals: 3,
        instructions: vec![
            Instruction::new(OpCode::NewObj, vec![0], 0), // 0: {}
            Instruction::with_operand(OpCode::PUSH, 0, 0), // 1: "x"
            Instruction::with_operand(OpCode::PUSH, 3, 0), // 2: 1
            Instruction::new(OpCode::SetField, vec![], 0),  // 3
            Instruction::with_operand(OpCode::STORE, 0, 0), // 4: obj
            
            Instruction::with_operand(OpCode::PUSH, 1, 0), // 5: i = 0
            Instruction::with_operand(OpCode::STORE, 1, 0), // 6
            Instruction::with_operand(OpCode::PUSH, 1, 0), // 7: sum = 0
            Instruction::with_operand(OpCode::STORE, 2, 0), // 8

            // Loop start (IP 9)
            Instruction::with_operand(OpCode::LOAD, 1, 0), // 9
            Instruction::with_operand(OpCode::PUSH, 2, 0), // 10: iterations
            Instruction::new(OpCode::GE, vec![], 0), // 11
            Instruction::with_operand(OpCode::JNZ, 24, 0), // 12: jump to end if i >= iterations (IP 24)
            
            Instruction::with_operand(OpCode::LOAD, 0, 0), // 13: obj
            Instruction::with_operand(OpCode::PUSH, 0, 0), // 14: "x"
            Instruction::new(OpCode::GetField, vec![], 0),  // 15
            Instruction::with_operand(OpCode::LOAD, 2, 0), // 16: sum
            Instruction::new(OpCode::ADD, vec![], 0),      // 17
            Instruction::with_operand(OpCode::STORE, 2, 0), // 18: sum += obj.x
            
            Instruction::with_operand(OpCode::LOAD, 1, 0), // 19
            Instruction::with_operand(OpCode::PUSH, 3, 0), // 20: 1
            Instruction::new(OpCode::ADD, vec![], 0),      // 21
            Instruction::with_operand(OpCode::STORE, 1, 0), // 22: i++
            
            Instruction::with_operand(OpCode::JMP, 9, 0), // 23: Loop back
            
            Instruction::with_operand(OpCode::LOAD, 2, 0), // 24
            Instruction::new(OpCode::RET, vec![], 0),       // 25
        ],
        constants: vec![
            Value::String("x".to_string()), 
            Value::Int(0), 
            Value::Int(iterations), 
            Value::Int(1)
        ],
        upvalues: vec![],
        line_info: vec![],
    };
    module.add_function(main_fn);
    module
}

#[test]
fn bench_numeric_jit() {
    let iterations = 1_000_000;
    let module = create_numeric_module(iterations);
    
    // Interpreter
    let mut config_int = VmConfig::default();
    config_int.enable_jit = false;
    let mut vm_int = NyxVm::new(config_int);
    vm_int.load(module.clone());
    let start_int = Instant::now();
    let res_int = vm_int.run("main").unwrap();
    let duration_int = start_int.elapsed();
    println!("Numeric Interpreter: {:?} (result: {:?})", duration_int, res_int);
    
    // JIT
    let mut config_jit = VmConfig::default();
    config_jit.enable_jit = true;
    let mut vm_jit = NyxVm::new(config_jit);
    vm_jit.load(module);
    let start_jit = Instant::now();
    let res_jit = vm_jit.run("main").unwrap();
    let duration_jit = start_jit.elapsed();
    println!("Numeric JIT:         {:?} (result: {:?})", duration_jit, res_jit);
    
    assert_eq!(res_int, res_jit);
}

#[test]
fn bench_ic_jit() {
    let iterations = 100_000;
    let module = create_ic_module(iterations);
    
    // Interpreter
    let mut config_int = VmConfig::default();
    config_int.enable_jit = false;
    let mut vm_int = NyxVm::new(config_int);
    vm_int.load(module.clone());
    let start_int = Instant::now();
    let res_int = vm_int.run("main").unwrap();
    let duration_int = start_int.elapsed();
    println!("IC Interpreter: {:?} (result: {:?})", duration_int, res_int);
    
    // JIT
    let mut config_jit = VmConfig::default();
    config_jit.enable_jit = true;
    let mut vm_jit = NyxVm::new(config_jit);
    vm_jit.load(module);
    let start_jit = Instant::now();
    let res_jit = vm_jit.run("main").unwrap();
    let duration_jit = start_jit.elapsed();
    println!("IC JIT:         {:?} (result: {:?})", duration_jit, res_jit);
    
    assert_eq!(res_int, res_jit);
    assert!(match res_jit { Value::Int(n) => n > 0, _ => false });
}

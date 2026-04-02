use nyx_vm::Value;

#[test]
fn test_value_tags() {
    let v_null = Value::Null;
    let v_bool = Value::Bool(true);
    let v_int = Value::Int(42);
    let v_float = Value::Float(3.14);
    let v_pointer = Value::Pointer(123);
    let v_unit = Value::Unit;

    unsafe {
        println!("Value size: {}", std::mem::size_of::<Value>());
        println!("Null tag: {:X}", *(&v_null as *const _ as *const u64));
        println!("Bool tag: {:X}", *(&v_bool as *const _ as *const u64));
        println!("Int tag: {:X}", *(&v_int as *const _ as *const u64));
        println!("Float tag: {:X}", *(&v_float as *const _ as *const u64));
        println!("Pointer tag: {:X}", *(&v_pointer as *const _ as *const u64));
        println!(
            "Pointer value at offset 8: {}",
            *((&v_pointer as *const _ as *const u64).add(1))
        );
        println!("Unit tag: {:X}", *(&v_unit as *const _ as *const u64));
    }
}

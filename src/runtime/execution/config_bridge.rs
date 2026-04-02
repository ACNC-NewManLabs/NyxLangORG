use crate::runtime::execution::nyx_vm::{NyxVm, Value, EvalError};

pub fn register_config_stdlib(vm: &mut NyxVm) {
    vm.register_native("std::config::get_version", get_version_native);
    vm.register_native("std::config::get_limit", get_limit_native);
}

/// Returns the current Nyx Runtime version.
pub fn get_version_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    Ok(Value::Str("1.0.0-production".to_string()))
}

/// Returns specific system limits (e.g., net_body_max).
pub fn get_limit_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if args.is_empty() {
        return Err(EvalError::new("get_limit(name) expects 1 argument".to_string()));
    }

    let limit_name = match &args[0] {
        Value::Str(s) => s.as_str(),
        _ => return Err(EvalError::new("Limit name must be a string".to_string())),
    };

    match limit_name {
        "net_body_max" => Ok(Value::Int(2 * 1024 * 1024)), // 2MB
        "net_header_max" => Ok(Value::Int(100)),
        "actor_mailbox_max" => Ok(Value::Int(1024)),
        "agent_chunk_max" => Ok(Value::Int(10000)),
        _ => Ok(Value::Null),
    }
}

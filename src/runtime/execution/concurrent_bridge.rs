use crate::runtime::execution::nyx_vm::{EvalError, NyxVm, Value};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::panic;
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread_local;

// Global Registry of all active Actor Senders
lazy_static! {
    static ref ACTOR_REGISTRY: Mutex<HashMap<u64, SyncSender<Value>>> = Mutex::new(HashMap::new());
}

// Global Counter for Actor IDs
static ACTOR_COUNTER: Mutex<u64> = Mutex::new(2); // 1 = Main/Root Actor

thread_local! {
    static MY_ACTOR_ID: OnceLock<u64> = OnceLock::new();
}

pub fn register_concurrent_stdlib(vm: &mut NyxVm) {
    vm.register_native("std::actor::spawn", spawn_actor_native);
    vm.register_native("std::actor::send", send_actor_native);
    vm.register_native("std::actor::receive", receive_actor_native);
    vm.register_native("std::actor::self", self_actor_native);

    // Initialize the Root Actor (ID 1) mailbox (Bounded at 1024 for production)
    let (tx, rx) = sync_channel::<Value>(1024);
    ACTOR_REGISTRY.lock().unwrap().insert(1, tx);
    vm.actor_mailbox = Some(Arc::new(Mutex::new(rx)));
}

/// Spawns a new Actor isolate running a specified function with panic isolation.
pub fn spawn_actor_native(vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if args.is_empty() {
        return Err(EvalError::new(
            "spawn(entry_func, args...) expects at least a function name".to_string(),
        ));
    }

    let entry_func = match &args[0] {
        Value::Str(s) => s.clone(),
        _ => {
            return Err(EvalError::new(
                "First argument must be the entry function name".to_string(),
            ))
        }
    };

    let func_args = if args.len() > 1 {
        args[1..].to_vec()
    } else {
        vec![]
    };

    // PRODUCTION: Bounded mailbox (1024) to prevent memory exhaustion
    let (tx, rx) = sync_channel::<Value>(1024);
    let id = {
        let mut counter = ACTOR_COUNTER.lock().unwrap();
        let id = *counter;
        *counter += 1;
        id
    };

    ACTOR_REGISTRY.lock().unwrap().insert(id, tx);

    // Clone the VM state (Isolate)
    let mut actor_vm = vm.clone_for_actor();
    actor_vm.actor_mailbox = Some(Arc::new(Mutex::new(rx)));

    // Spawn in a new OS thread with PANIC ISOLATION
    std::thread::spawn(move || {
        // Init thread-local ID
        MY_ACTOR_ID.with(|cell| {
            let _ = cell.set(id);
        });

        let result = panic::catch_unwind(panic::AssertUnwindSafe(move || {
            let _ = actor_vm.call_function(&entry_func, func_args);
        }));

        if let Err(_) = result {
            log::error!(
                "[Nyx-Concurrent] Actor {} panicked! Isolation maintained.",
                id
            );
        }

        // PRODUCTION: Guaranteed Registry Cleanup
        let mut registry = ACTOR_REGISTRY.lock().unwrap();
        registry.remove(&id);
        log::info!("[Nyx-Concurrent] Actor {} terminated and reaped.", id);
    });

    Ok(Value::Int(id as i64))
}

/// Sends a message (Nyx Value) to a specific Actor.
pub fn send_actor_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if args.len() < 2 {
        return Err(EvalError::new(
            "send(actor_id, message) expects 2 arguments".to_string(),
        ));
    }

    let target_id = match args[0] {
        Value::Int(i) => i as u64,
        _ => {
            return Err(EvalError::new(
                "First argument must be an integer Actor ID".to_string(),
            ))
        }
    };

    let msg = args[1].clone();

    let registry = ACTOR_REGISTRY.lock().unwrap();
    if let Some(tx) = registry.get(&target_id) {
        // PRODUCTION: try_send to avoid blocking the sender if the mailbox is full
        match tx.try_send(msg) {
            Ok(_) => Ok(Value::Bool(true)),
            Err(std::sync::mpsc::TrySendError::Full(_)) => {
                log::warn!(
                    "[Nyx-Concurrent] Actor {} mailbox is full! Message dropped.",
                    target_id
                );
                Ok(Value::Bool(false))
            }
            Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                log::warn!("[Nyx-Concurrent] Actor {} is disconnected.", target_id);
                Ok(Value::Bool(false))
            }
        }
    } else {
        Ok(Value::Bool(false))
    }
}

/// Receives a message from the current actor's mailbox (blocks).
pub fn receive_actor_native(vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    if let Some(mailbox_lock) = &vm.actor_mailbox {
        let rx = mailbox_lock.lock().unwrap();

        // Blocking wait
        if let Ok(msg) = rx.recv() {
            Ok(msg)
        } else {
            // Channel closed
            Ok(Value::Null)
        }
    } else {
        Err(EvalError::new(
            "Current context does not have an actor mailbox.".to_string(),
        ))
    }
}

/// Returns the current Actor's unique ID.
pub fn self_actor_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    let id = MY_ACTOR_ID.with(|cell| *cell.get().unwrap_or(&1));
    Ok(Value::Int(id as i64))
}

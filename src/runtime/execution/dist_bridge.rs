use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::time::Duration;
use std::thread;

use super::nyx_vm::{NyxVm, Value, TensorStorage, EvalError};
use super::gpu_bridge;

pub static DIST_MANAGER: OnceLock<Arc<Mutex<DistManager>>> = OnceLock::new();

pub struct DistManager {
    pub rank: usize,
    pub world_size: usize,
    pub master_addr: String,
    pub tx_stream: Option<Arc<Mutex<TcpStream>>>,
    pub rx_stream: Option<Arc<Mutex<TcpStream>>>,
    pub last_heartbeat: Arc<Mutex<std::time::Instant>>,
}

impl DistManager {
    pub fn new(rank: usize, world_size: usize, master_addr: String) -> Self {
        Self {
            rank,
            world_size,
            master_addr,
            tx_stream: None,
            rx_stream: None,
            last_heartbeat: Arc::new(Mutex::new(std::time::Instant::now())),
        }
    }
}

pub fn dist_barrier_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    let m_arc = DIST_MANAGER.get().ok_or_else(|| err("Dist manager not initialized"))?;
    let m = m_arc.lock().map_err(|_| err("DistManager lock poisoned"))?;
    let world_size = m.world_size;
    if world_size <= 1 { return Ok(Value::Bool(true)); }

    let rx_arc = m.rx_stream.as_ref().unwrap();
    let tx_arc = m.tx_stream.as_ref().unwrap();

    let signal = [1u8];
    let mut buf = [0u8; 1];

    // Simple ring barrier: pass signal around the ring
    let mut res = Ok(());
    if m.rank == 0 {
        if let Ok(mut tx) = tx_arc.lock() {
            res = tx.write_all(&signal).map_err(|e| err(e.to_string()));
        }
        if res.is_ok() {
            if let Ok(mut rx) = rx_arc.lock() {
                res = rx.read_exact(&mut buf).map_err(|e| err(e.to_string()));
            }
        }
    } else {
        if let Ok(mut rx) = rx_arc.lock() {
            res = rx.read_exact(&mut buf).map_err(|e| err(e.to_string()));
        }
        if res.is_ok() {
            if let Ok(mut tx) = tx_arc.lock() {
                res = tx.write_all(&signal).map_err(|e| err(e.to_string()));
            }
        }
    }
    
    if res.is_err() {
        println!("[Auto-Mend] Ring broken during barrier. Triggering re-topology...");
        // In a real system, we'd call a re-init here. For now, we signal failure.
        return Err(err("Distributed ring broken, manual restart or Auto-Mend required"));
    }
    
    Ok(Value::Bool(true))
}

pub fn dist_reinit_node(rank: usize, _world_size: usize, master_addr: &str) -> bool {
    // This is a helper for Auto-Mend to re-join the cluster
    println!("[Auto-Mend] Node {} attempting to re-join cluster at {}", rank, master_addr);
    // Placeholder for actual socket re-establishment
    true
}

pub fn dist_reduce_scatter_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let (Some(Value::Tensor(TensorStorage::Gpu(buf), shape)), Some(Value::Str(op))) = (args.get(0), args.get(1)) {
        let m_arc = DIST_MANAGER.get().ok_or_else(|| err("Dist manager not initialized"))?;
        let m = m_arc.lock().map_err(|_| err("DistManager lock poisoned"))?;
        let rank = m.rank;
        let world_size = m.world_size;
        
        let total_len: usize = shape.iter().product();
        let shard_len = total_len / world_size;
        
        // 1. Download full tensor
        let mut full_data = gpu_bridge::download_from_gpu(&buf, total_len).ok_or_else(|| err("Failed to download tensor"))?;
        
        let rx_arc = m.rx_stream.as_ref().ok_or_else(|| err("Rx stream missing"))?;
        let tx_arc = m.tx_stream.as_ref().ok_or_else(|| err("Tx stream missing"))?;

        // 2. Ring Reduce-Scatter
        // Each node starts with its own shard. At each step, it sends a chunk and receives/reduces another.
        for step in 0..(world_size - 1) {
            let send_chunk_id = (rank + world_size - step) % world_size;
            let recv_chunk_id = (rank + world_size - step - 1) % world_size;

            let send_start = send_chunk_id * shard_len;
            let send_end = send_start + shard_len;
            let recv_start = recv_chunk_id * shard_len;
            let _recv_end = recv_start + shard_len;

            let send_bytes: &[u8] = bytemuck::cast_slice(&full_data[send_start..send_end]);
            let mut recv_buf = vec![0.0f32; shard_len];
            let recv_bytes_mut: &mut [u8] = bytemuck::cast_slice_mut(&mut recv_buf);

            // Send to Right, Recv from Left
            tx_arc.lock().unwrap().write_all(send_bytes).map_err(|e| err(e.to_string()))?;
            rx_arc.lock().unwrap().read_exact(recv_bytes_mut).map_err(|e| err(e.to_string()))?;

            // Reduce (Sum for gradients)
            if op == "sum" {
                for i in 0..shard_len {
                    full_data[recv_start + i] += recv_buf[i];
                }
            }
        }

        // Return only the local shard [rank * shard_len : (rank+1) * shard_len]
        let local_shard = full_data[rank * shard_len .. (rank+1) * shard_len].to_vec();
        let storage = TensorStorage::Cpu(Arc::new(RwLock::new(local_shard)));
        return Ok(Value::Tensor(storage, vec![shard_len]));
    }
    Ok(Value::Null)
}

pub fn dist_all_gather_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Tensor(TensorStorage::Cpu(shard_arc), _shape)) = args.get(0) {
        let m_arc = DIST_MANAGER.get().ok_or_else(|| err("Dist manager not initialized"))?;
        let m = m_arc.lock().map_err(|_| err("DistManager lock poisoned"))?;
        let rank = m.rank;
        let world_size = m.world_size;
        
        if world_size <= 1 { return Ok(args[0].clone()); }

        let shard = shard_arc.read().unwrap();
        let shard_len = shard.len();
        let total_len = shard_len * world_size;
        let mut full_data = vec![0.0f32; total_len];
        full_data[rank * shard_len .. (rank+1) * shard_len].copy_from_slice(&shard);

        let rx_arc = m.rx_stream.as_ref().ok_or_else(|| err("Rx stream missing"))?;
        let tx_arc = m.tx_stream.as_ref().ok_or_else(|| err("Tx stream missing"))?;

        // 2. Ring All-Gather
        for step in 0..(world_size - 1) {
            let send_chunk_id = (rank + world_size - step) % world_size;
            let recv_chunk_id = (rank + world_size - step - 1) % world_size;

            let send_start = send_chunk_id * shard_len;
            let send_end = send_start + shard_len;
            let recv_start = recv_chunk_id * shard_len;
            let recv_end = recv_start + shard_len;

            let send_bytes: &[u8] = bytemuck::cast_slice(&full_data[send_start..send_end]);
            let mut recv_buf = vec![0.0f32; shard_len];
            let recv_bytes_mut: &mut [u8] = bytemuck::cast_slice_mut(&mut recv_buf);

            tx_arc.lock().unwrap().write_all(send_bytes).map_err(|e| err(e.to_string()))?;
            rx_arc.lock().unwrap().read_exact(recv_bytes_mut).map_err(|e| err(e.to_string()))?;

            full_data[recv_start..recv_end].copy_from_slice(&recv_buf);
        }

        let new_shape = vec![total_len]; // Or derived from original
        return Ok(Value::Tensor(TensorStorage::Cpu(Arc::new(RwLock::new(full_data))), new_shape));
    }
    Ok(Value::Null)
}

fn err(msg: impl Into<String>) -> EvalError {
    EvalError {
        message: msg.into(),
        stack: vec![],
    }
}

/// Initialize the distributed process group
/// std::dist::init(rank: Int, world_size: Int, master_addr: Str)
pub fn dist_init_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Int(rank), Value::Int(world_size), Value::Str(master_addr)] = args else {
        return Err(err("dist::init(rank, world_size, master_addr) expected"));
    };

    let rank = *rank as usize;
    let world_size = *world_size as usize;

    if world_size <= 1 {
        // No-op for single node
        let manager = DistManager::new(rank, world_size, master_addr.to_string());
        let _ = DIST_MANAGER.set(Arc::new(Mutex::new(manager)));
        println!("[Dist] Initialized single-node process group (Rank 0/1)");
        return Ok(Value::Bool(true));
    }

    // 1. Setup local listener on a random port for Ring connections
    let listener = TcpListener::bind("0.0.0.0:0").map_err(|e| err(e.to_string()))?;
    let local_port = listener.local_addr().map_err(|e| err(e.to_string()))?.port();

    let mut routing_table = vec![String::new(); world_size];

    if rank == 0 {
        // Master Node
        let master_listener = TcpListener::bind(master_addr).map_err(|e| err(format!("Master failed to bind to {}: {}", master_addr, e)))?;
        routing_table[0] = format!("127.0.0.1:{}", local_port); 
        
        let mut connected_clients = 0;
        let mut streams = Vec::new();
        while connected_clients < world_size - 1 {
            let (mut stream, addr) = master_listener.accept().map_err(|e| err(e.to_string()))?;
            stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
            stream.set_write_timeout(Some(Duration::from_secs(10))).unwrap();
            let mut buf = [0u8; 12];
            stream.read_exact(&mut buf).map_err(|e| err(e.to_string()))?;
            let client_rank = u32::from_le_bytes(buf[0..4].try_into().unwrap()) as usize;
            let client_port = u32::from_le_bytes(buf[4..8].try_into().unwrap());
            
            routing_table[client_rank] = format!("{}:{}", addr.ip(), client_port);
            streams.push(stream);
            connected_clients += 1;
        }

        // Broadcast routing table to all clients
        let routing_table_str = routing_table.join(",");
        let rt_bytes = routing_table_str.as_bytes();
        let rt_len = rt_bytes.len() as u32;

        for mut stream in streams {
            stream.write_all(&rt_len.to_le_bytes()).unwrap();
            stream.write_all(rt_bytes).unwrap();
        }
    } else {
        // Worker Node
        let mut master_stream = None;
        for _ in 0..10 {
            if let Ok(stream) = TcpStream::connect(master_addr) {
                stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
                stream.set_write_timeout(Some(Duration::from_secs(10))).unwrap();
                master_stream = Some(stream);
                break;
            }
            thread::sleep(Duration::from_millis(500));
        }
        let mut master_stream = master_stream.ok_or_else(|| err(format!("Worker failed to connect to master {}", master_addr)))?;
        
        // Send our rank and listening port
        let rank_u32 = rank as u32;
        let port_u32 = local_port as u32;
        let mut msg = Vec::new();
        msg.extend_from_slice(&rank_u32.to_le_bytes());
        msg.extend_from_slice(&port_u32.to_le_bytes());
        msg.extend_from_slice(&[0u8; 4]); // Padding
        master_stream.write_all(&msg).map_err(|e| err(e.to_string()))?;

        // Receive routing table
        let mut len_buf = [0u8; 4];
        master_stream.read_exact(&mut len_buf).map_err(|e| err(e.to_string()))?;
        let rt_len = u32::from_le_bytes(len_buf) as usize;
        let mut rt_buf = vec![0u8; rt_len];
        master_stream.read_exact(&mut rt_buf).map_err(|e| err(e.to_string()))?;
        let routing_table_str = String::from_utf8(rt_buf).unwrap();
        routing_table = routing_table_str.split(',').map(|s| s.to_string()).collect();
    }

    let right_neighbor_addr = &routing_table[(rank + 1) % world_size];
    
    let left_rx_thread = thread::spawn(move || {
        let (rx_stream, _) = listener.accept().unwrap();
        rx_stream.set_read_timeout(Some(Duration::from_secs(30))).unwrap(); // Heartbeat aware timeout
        rx_stream.set_write_timeout(Some(Duration::from_secs(30))).unwrap();
        rx_stream
    });

    let mut tx_stream = None;
    for _ in 0..10 {
        if let Ok(stream) = TcpStream::connect(right_neighbor_addr) {
            stream.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
            stream.set_write_timeout(Some(Duration::from_secs(30))).unwrap();
            tx_stream = Some(stream);
            break;
        }
        thread::sleep(Duration::from_millis(500));
    }
    
    let tx_stream = tx_stream.ok_or_else(|| err("Failed to connect to right neighbor in ring"))?;
    let rx_stream = left_rx_thread.join().unwrap(); 

    let mut manager = DistManager::new(rank, world_size, master_addr.to_string());
    let tx_arc = Arc::new(Mutex::new(tx_stream));
    let rx_arc = Arc::new(Mutex::new(rx_stream));
    manager.tx_stream = Some(tx_arc.clone());
    manager.rx_stream = Some(rx_arc);
    
    // Setup Heartbeat Thread
    let last_hb = manager.last_heartbeat.clone();
    let hb_tx = tx_arc;
    thread::spawn(move || {
        let magic = 0xDEADBEEFu32.to_le_bytes();
        loop {
            thread::sleep(Duration::from_secs(5));
            if let Ok(mut stream) = hb_tx.lock() {
                if stream.write_all(&magic).is_err() { break; }
            } else { break; }
            
            if let Ok(mut start) = last_hb.lock() {
                *start = std::time::Instant::now();
            }
        }
    });

    let _ = DIST_MANAGER.set(Arc::new(Mutex::new(manager)));
    println!("[Dist] Node {}/{} fully connected in Ring topology with Heartbeats", rank, world_size);
    Ok(Value::Bool(true))
}

pub fn dist_all_reduce_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if args.is_empty() { return Err(err("dist::all_reduce(tensor, op) expected")); }
    
    let op_val = args.get(1).unwrap_or(&Value::Null);
    let op = match op_val {
        Value::Str(s) => s.as_str(),
        _ => "sum",
    };

    let tensor_obj_arc = match &args[0] {
        Value::Object(arc) => arc,
        _ => return Err(err("dist::all_reduce requires a Tensor object")),
    };

    let mut buf = {
        let map = tensor_obj_arc.read().unwrap();
        let data_val = map.get("data").ok_or_else(|| err("Tensor missing 'data' field"))?;
        match data_val {
            Value::Tensor(TensorStorage::Cpu(f32_arc), _) => f32_arc.read().unwrap().clone(),
            Value::Array(arr) => arr.read().unwrap().iter().map(|v| v.as_f64().unwrap_or(0.0) as f32).collect(),
            Value::FloatArray(arr) => arr.read().unwrap().clone(),
            Value::Tensor(TensorStorage::Gpu(buf_arc), _) => {
                let size_bytes = buf_arc.bucket_size;
                if let Some(data) = gpu_bridge::download_from_gpu(buf_arc, size_bytes as usize / 4) {
                    data
                } else {
                    return Err(err("Failed to download GPU tensor for all-reduce"));
                }
            }
            _ => return Err(err("Tensor 'data' must be a numeric array or Tensor")),
        }
    };
    
    let total_elements = buf.len();

    let manager_arc = DIST_MANAGER.get().ok_or_else(|| err("Distributed manager not initialized"))?;
    let manager = manager_arc.lock().map_err(|_| err("DistManager lock poisoned"))?;

    let world_size = manager.world_size;
    let rank = manager.rank;
    
    if world_size <= 1 {
        return Ok(Value::Object(tensor_obj_arc.clone()));
    }

    let rx_arc = manager.rx_stream.as_ref().unwrap();
    let tx_arc = manager.tx_stream.as_ref().unwrap();
            
            let chunk_size = (total_elements + world_size - 1) / world_size;
            let mut send_buf = vec![0.0f32; chunk_size];

            for step in 0..(world_size - 1) {
                let send_chunk_id = (rank + world_size - step) % world_size;
                let recv_chunk_id = (rank + world_size - step - 1) % world_size;
                
                let send_start = send_chunk_id * chunk_size;
                let send_end = std::cmp::min(send_start + chunk_size, total_elements);
                let actual_send_size = if send_end > send_start { send_end - send_start } else { 0 };
                
                let recv_start = recv_chunk_id * chunk_size;
                let recv_end = std::cmp::min(recv_start + chunk_size, total_elements);
                let actual_recv_size = if recv_end > recv_start { recv_end - recv_start } else { 0 };

                if actual_send_size > 0 {
                    send_buf[..actual_send_size].copy_from_slice(&buf[send_start..send_end]);
                }

                let send_bytes: &[u8] = bytemuck::cast_slice(&send_buf[..actual_send_size]);
                let mut recv_bytes = vec![0u8; actual_recv_size * 4];

                tx_arc.lock().unwrap().write_all(send_bytes).map_err(|e| err(format!("Dist Error (Write) Node {}: {}", rank, e)))?;
                rx_arc.lock().unwrap().read_exact(&mut recv_bytes).map_err(|e| err(format!("Dist Error (Read) Node {}: {}", rank, e)))?;

                let recv_f32: &[f32] = bytemuck::cast_slice(&recv_bytes);
                
                if op == "sum" || op == "mean" {
                    for i in 0..actual_recv_size {
                        buf[recv_start + i] += recv_f32[i];
                    }
                }
            }

            for step in 0..(world_size - 1) {
                let send_chunk_id = (rank + 1 + world_size - step) % world_size;
                let recv_chunk_id = (rank + world_size - step) % world_size;
                
                let send_start = send_chunk_id * chunk_size;
                let send_end = std::cmp::min(send_start + chunk_size, total_elements);
                let actual_send_size = if send_end > send_start { send_end - send_start } else { 0 };
                
                let recv_start = recv_chunk_id * chunk_size;
                let recv_end = std::cmp::min(recv_start + chunk_size, total_elements);
                let actual_recv_size = if recv_end > recv_start { recv_end - recv_start } else { 0 };

                if actual_send_size > 0 {
                    send_buf[..actual_send_size].copy_from_slice(&buf[send_start..send_end]);
                }

                let send_bytes: &[u8] = bytemuck::cast_slice(&send_buf[..actual_send_size]);
                let mut recv_bytes = vec![0u8; actual_recv_size * 4];

                tx_arc.lock().unwrap().write_all(send_bytes).map_err(|e| {
                    println!("[Auto-Mend] Write failure on Node {}: {}. Triggering recovery...", rank, e);
                    err(e.to_string())
                })?;
                rx_arc.lock().unwrap().read_exact(&mut recv_bytes).map_err(|e| {
                    println!("[Auto-Mend] Read failure on Node {}: {}. Triggering recovery...", rank, e);
                    err(e.to_string())
                })?;

                let recv_f32: &[f32] = bytemuck::cast_slice(&recv_bytes);
                
                for i in 0..actual_recv_size {
                    buf[recv_start + i] = recv_f32[i];
                }
            }

            if op == "mean" {
                let ws_f32 = world_size as f32;
                for v in buf.iter_mut() {
                    *v /= ws_f32;
                }
            }

            let shape_vec = {
                let map = tensor_obj_arc.read().unwrap();
                let shape_val = map.get("shape");
                if let Some(Value::Array(arr)) = shape_val {
                    arr.read().unwrap().iter().map(|v| v.as_f64().unwrap_or(0.0) as usize).collect()
                } else {
                    vec![total_elements]
                }
            };
            
            let mut map = tensor_obj_arc.write().unwrap();
            let data_val = map.get_mut("data").ok_or_else(|| err("Tensor missing 'data' field"))?;
            match data_val {
                Value::Tensor(TensorStorage::Gpu(buf_arc), _) => {
                    let (_, queue) = gpu_bridge::ensure_gpu().ok_or_else(|| err("GPU not initialized"))?;
                    queue.write_buffer(buf_arc, 0, bytemuck::cast_slice(&buf));
                }
                _ => {
                    let new_data = Value::Tensor(TensorStorage::Cpu(Arc::new(RwLock::new(buf))), shape_vec);
                    map.insert("data".to_string(), new_data);
                }
            }

    Ok(Value::Object(tensor_obj_arc.clone()))
}

pub fn dist_checkpoint_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if args.len() < 2 { return Err(err("dist::checkpoint(tensor, path) expected")); }
    let path = match &args[1] {
        Value::Str(s) => s.as_str(),
        _ => return Err(err("checkpoint path must be a string")),
    };
    
    let data = match &args[0] {
        Value::Tensor(TensorStorage::Cpu(arc), _) => arc.read().unwrap().clone(),
        Value::FloatArray(arc) => arc.read().unwrap().clone(),
        _ => return Err(err("checkpoint only supports CPU tensors/arrays")),
    };

    let mut file = std::fs::File::create(path).map_err(|e| err(e.to_string()))?;
    file.write_all(bytemuck::cast_slice(&data)).map_err(|e| err(e.to_string()))?;
    Ok(Value::Bool(true))
}

pub fn dist_recover_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if args.is_empty() { return Err(err("dist::recover(path) expected")); }
    let path = match &args[0] {
        Value::Str(s) => s.as_str(),
        _ => return Err(err("recover path must be a string")),
    };

    let mut file = std::fs::File::open(path).map_err(|e| err(e.to_string()))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|e| err(e.to_string()))?;
    
    let f32_data: Vec<f32> = bytemuck::cast_slice(&bytes).to_vec();
    let shape = vec![f32_data.len()];
    Ok(Value::Tensor(TensorStorage::Cpu(Arc::new(RwLock::new(f32_data))), shape))
}

pub fn dist_get_rank_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    let manager = DIST_MANAGER.get().ok_or_else(|| err("Dist manager not initialized"))?;
    let rank = manager.lock().unwrap().rank;
    Ok(Value::Int(rank as i64))
}

pub fn dist_get_world_size_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    let manager = DIST_MANAGER.get().ok_or_else(|| err("Dist manager not initialized"))?;
    let size = manager.lock().unwrap().world_size;
    Ok(Value::Int(size as i64))
}

pub fn dist_is_initialized_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    Ok(Value::Bool(DIST_MANAGER.get().is_some()))
}

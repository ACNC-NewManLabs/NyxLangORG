use crate::runtime::execution::nyx_vm::{EvalError, NyxVm, Value};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::thread;
// use std::net::TcpListener as _StdTcpListener; // Fixed unused import

pub fn register_net_stdlib(vm: &mut NyxVm) {
    vm.register_native("__native_net_http_serve", http_serve_native);
}

pub fn http_serve_native(vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let port = match args.first() {
        Some(Value::Int(p)) => *p as u16,
        _ => {
            return Err(EvalError::new(
                "Expected port as first argument".to_string(),
            ))
        }
    };

    let handler_name = match args.get(1) {
        Some(Value::Str(s)) => s.clone(),
        _ => {
            return Err(EvalError::new(
                "Expected handler function name as second argument".to_string(),
            ))
        }
    };

    log::info!(
        "[Nyx-Net] Starting High-Concurrency HTTP Event Loop on 0.0.0.0:{}",
        port
    );

    // Create an MPSC channel to send completed requests back to the main Nyx thread
    let (req_tx, req_rx) = std::sync::mpsc::channel::<(Value, std::sync::mpsc::Sender<String>)>();

    // Spawn a dedicated native thread to run the Tokio async asynchronous I/O pool
    thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to build Tokio Nyx-Net pool");

        rt.block_on(async move {
            let addr = format!("0.0.0.0:{}", port);
            let listener = tokio::net::TcpListener::bind(&addr).await.expect("Failed to bind TCP port");

            loop {
                // Accept incoming connections concurrently
                let (socket, peer_addr) = match listener.accept().await {
                    Ok(res) => res,
                    Err(e) => {
                        log::error!("[Nyx-Net] Backend Accept Error: {}", e);
                        continue;
                    }
                };
                let tx_clone = req_tx.clone();
                let client_ip = peer_addr.ip().to_string();

                // Spawn an ultra-lightweight green thread for each network connection
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    use tokio::time::{timeout, Duration};

                    let mut socket = socket;
                    let mut buf = [0; 65536]; // 64KB read buffer
                    let read_result = timeout(Duration::from_secs(30), socket.read(&mut buf)).await;

                    match read_result {
                        Ok(Ok(n)) => {
                            if n == 0 { return; } // Client closed

                            // PRODUCTION: Size Limit Check
                            if n > 2 * 1024 * 1024 { // 2MB Hard Limit
                                log::warn!("[Nyx-Net] Body size limit exceeded from {}", client_ip);
                                let _ = socket.write_all(b"HTTP/1.1 413 Payload Too Large\r\n\r\n").await;
                                return;
                            }

                            let req_str = String::from_utf8_lossy(&buf[..n]);

                            // Parse HTTP natively
                            let mut lines = req_str.lines();
                            if let Some(first_line) = lines.next() {
                                let parts: Vec<&str> = first_line.split_whitespace().collect();
                                if parts.len() >= 2 {
                                    let _method = parts[0];
                                    let path = parts[1];

                                    let mut headers = HashMap::new();
                                    let mut body = String::new();
                                    let mut in_body = false;
                                    let mut header_count = 0;

                                    for line in lines {
                                        if in_body {
                                            body.push_str(line);
                                            body.push('\n');
                                        } else if line.is_empty() {
                                            in_body = true;
                                        } else if let Some(idx) = line.find(':') {
                                            // PRODUCTION: Header Limit Check
                                            if header_count > 100 { break; }
                                            header_count += 1;

                                            let key = line[..idx].trim().to_lowercase();
                                            let val = line[idx+1..].trim().to_string();
                                            headers.insert(key, Value::Str(val));
                                        }
                                    }

                                    // Build Nyx Request Object Hashmap
                                    let mut req_map = HashMap::new();
                                    req_map.insert("method".to_string(), Value::Str(_method.to_string()));
                                    req_map.insert("path".to_string(), Value::Str(path.to_string()));
                                    req_map.insert("body".to_string(), Value::Str(body.trim_end().to_string()));
                                    req_map.insert("ip".to_string(), Value::Str(client_ip));
                                    req_map.insert("headers".to_string(), Value::Object(Arc::new(RwLock::new(headers))));

                                    let req_val = Value::Object(Arc::new(RwLock::new(req_map)));

                                    // Create a single-shot reply channel for this socket
                                    let (reply_tx, reply_rx) = std::sync::mpsc::channel();

                                    // Submit to Nyx VM Core Thread
                                    if let Err(e) = tx_clone.send((req_val, reply_tx)) {
                                        log::error!("[Nyx-Net] VM channel failure: {}", e);
                                        return;
                                    }

                                    // Wait for VM logic with timeout (Script Execution Timeout: 60s)
                                    match reply_rx.recv_timeout(Duration::from_secs(60)) {
                                        Ok(response_json) => {
                                            let http_response = format!(
                                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nServer: Nyx-Production/v1.0\r\nConnection: close\r\n\r\n{}",
                                                response_json.len(),
                                                response_json
                                            );
                                            let _ = timeout(Duration::from_secs(10), socket.write_all(http_response.as_bytes())).await;
                                            let _ = socket.flush().await;
                                        },
                                        Err(_) => {
                                            log::error!("[Nyx-Net] Script handler timeout or crash for {}", path);
                                            let _ = socket.write_all(b"HTTP/1.1 504 Gateway Timeout\r\n\r\n").await;
                                        }
                                    }
                                }
                            }
                        },
                        Ok(Err(e)) => log::error!("[Nyx-Net] Socket Read Error: {}", e),
                        Err(_) => log::warn!("[Nyx-Net] Request Receive Timeout from {}", client_ip),
                    }
                });
            }
        });
    });

    // Nyx Main Thread Event Loop
    // Process requests synchronously one-by-one so the script engine remains thread-safe while networking is 100% concurrent.
    while let Ok((req_obj, reply_channel)) = req_rx.recv() {
        // Execute the user's handler
        match vm.call_function(&handler_name, vec![req_obj]) {
            Ok(val) => {
                // Convert returned value to JSON
                let json_res = match &val {
                    Value::Object(o) => {
                        let map = o.read().unwrap_or_else(|e| e.into_inner());
                        serde_json::to_string(&*map).unwrap_or_else(|_| "{}".to_string())
                    }
                    Value::Str(s) => s.clone(), // Raw string response (HTML, etc.)
                    _ => serde_json::to_string(&val).unwrap_or_else(|_| "{}".to_string()),
                };

                if let Err(e) = reply_channel.send(json_res) {
                    log::error!(
                        "[Nyx-Net] Failed to send response back to socket thread: {}",
                        e
                    );
                }
            }
            Err(e) => {
                log::error!("[Nyx-Net] Script Handler Error: {}", e.message);
                let err_res = format!(
                    "{{\"error\": \"Internal Server Error\", \"details\": \"{}\"}}",
                    e.message
                );
                let _ = reply_channel.send(err_res);
            }
        }
    }

    Ok(Value::Null)
}

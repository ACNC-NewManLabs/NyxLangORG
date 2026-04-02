use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use crate::runtime::execution::df_engine::global_catalog;

pub struct NyxDriverMock;

impl NyxDriverMock {
    /// Starts a mock Arrow-over-TCP responder on the given port.
    /// This simulates a real database wire protocol for JDBC/ODBC compatibility.
    pub fn start(port: u16) -> std::io::Result<()> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port))?;
        println!("[Nyx-Driver] Mock Arrow-over-TCP server listening on port {}", port);

        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    let mut buffer = [0u8; 1024];
                    if let Ok(n) = stream.read(&mut buffer) {
                        let request = String::from_utf8_lossy(&buffer[..n]);
                        if request.starts_with("GET_ARROW_CHUNK") {
                            Self::handle_get_chunk(&mut stream, &request);
                        } else if request.starts_with("LIST_TABLES") {
                            Self::handle_list_tables(&mut stream);
                        }
                    }
                }
                Err(e) => eprintln!("[Nyx-Driver] Connection failed: {}", e),
            }
        }
        Ok(())
    }

    fn handle_get_chunk(stream: &mut TcpStream, request: &str) {
        let parts: Vec<&str> = request.split_whitespace().collect();
        if parts.len() < 2 { return; }
        let table_name = parts[1];

        let catalog = global_catalog().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(chunks) = catalog.get(table_name) {
            if !chunks.is_empty() {
                // In a real implementation, we would use arrow::ipc::writer::StreamWriter
                // Here we simulate the binary payload
                let _ = stream.write_all(b"ARROW_STREAM_START\n");
                let _ = stream.write_all(format!("CHUNK_COUNT: {}\n", chunks.len()).as_bytes());
                let _ = stream.write_all(b"BINARY_PAYLOAD_Omitted_for_Mock\n");
            }
        } else {
            let _ = stream.write_all(b"ERROR: Table not found\n");
        }
    }

    fn handle_list_tables(stream: &mut TcpStream) {
        let catalog = global_catalog().lock().unwrap_or_else(|e| e.into_inner());
        let tables: Vec<String> = catalog.keys().cloned().collect();
        let _ = stream.write_all(format!("TABLES: {}\n", tables.join(", ")).as_bytes());
    }
}

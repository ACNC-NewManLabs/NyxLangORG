use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::runtime::execution::sql_planner::SqlPlanner;
use crate::runtime::execution::df_engine::{create_physical_plan, export_to_arrow};

pub struct NyxServer;

impl NyxServer {
    /// Starts the production Nyx Arrow-over-TCP server.
    /// This server receives SQL queries, executes them using the vectorized engine,
    /// and streams back results in standard Apache Arrow IPC format.
    pub async fn start(port: u16) -> tokio::io::Result<()> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
        println!("[Nyx-Server] Production Arrow-over-TCP server listening on port {}", port);

        loop {
            match listener.accept().await {
                Ok((mut stream, addr)) => {
                    println!("[Nyx-Server] Accepted connection from {}", addr);
                    tokio::spawn(async move {
                        if let Err(e) = Self::process_connection(&mut stream).await {
                            eprintln!("[Nyx-Server] Error processing connection from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => eprintln!("[Nyx-Server] Accept error: {}", e),
            }
        }
    }

    async fn process_connection(stream: &mut TcpStream) -> tokio::io::Result<()> {
        let mut buffer = [0u8; 8192];
        loop {
            let n = stream.read(&mut buffer).await?;
            if n == 0 { break; } // Connection closed

            let sql = String::from_utf8_lossy(&buffer[..n]);
            println!("[Nyx-Server] Received SQL: {}", sql.trim());

            match Self::execute_and_serialize(&sql).await {
                Ok(data) => {
                    // Send Arrow IPC Stream
                    stream.write_all(&data).await?;
                }
                Err(e) => {
                    let error_msg = format!("ERROR: {}\n", e);
                    stream.write_all(error_msg.as_bytes()).await?;
                }
            }
        }
        Ok(())
    }

    async fn execute_and_serialize(sql: &str) -> Result<Vec<u8>, String> {
        let mut planner = SqlPlanner::new();
        let logical_plan = planner.plan(sql)?;
        
        let mut ctx = crate::runtime::execution::df_engine::ExecutionContext {
            sources: std::collections::HashMap::new(),
        };
        let mut physical_plan = create_physical_plan(&logical_plan, &mut ctx)?;

        let mut chunks = Vec::new();
        while let Some(chunk) = physical_plan.next_chunk()? {
            chunks.push(chunk);
        }

        if chunks.is_empty() {
             // Return an empty Arrow stream with correct schema
             return export_to_arrow(&[], &physical_plan.schema());
        }

        export_to_arrow(&chunks, &physical_plan.schema())
    }
}

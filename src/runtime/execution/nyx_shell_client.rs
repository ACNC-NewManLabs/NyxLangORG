use std::io::{self, Write};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub async fn run_shell(host: &str, port: u16) -> io::Result<()> {
    let addr = format!("{}:{}", host, port);
    println!("--- Nyx DB Interactive Shell (v0.1-beta) ---");
    println!("Connecting to Nyx-Server at {}...", addr);

    let mut stream = match TcpStream::connect(&addr).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to connect to {}: {}", addr, e);
            return Err(e);
        }
    };

    println!("Connected! Type your SQL queries below. (Type 'exit' to quit)");

    let mut input = String::new();
    loop {
        print!("nyx> ");
        io::stdout().flush()?;

        input.clear();
        io::stdin().read_line(&mut input)?;
        let sql = input.trim();

        if sql == "exit" || sql == "quit" {
            break;
        }
        if sql.is_empty() {
            continue;
        }

        // Send Query
        if let Err(e) = stream.write_all(sql.as_bytes()).await {
            eprintln!("Error sending query: {}", e);
            break;
        }

        // Read Response (Arrow Stream or Error)
        let mut buffer = vec![0u8; 65536];
        match stream.read(&mut buffer).await {
            Ok(0) => {
                println!("Server closed connection.");
                break;
            }
            Ok(n) => {
                let response = &buffer[..n];
                if response.starts_with(b"ERROR:") {
                    println!("{}", String::from_utf8_lossy(response));
                } else {
                    println!("[Shell] Received {} bytes of Arrow IPC data.", n);
                    if response.len() > 100 {
                        println!("[Shell] Query executed successfully.");
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading response: {}", e);
                break;
            }
        }
    }

    Ok(())
}

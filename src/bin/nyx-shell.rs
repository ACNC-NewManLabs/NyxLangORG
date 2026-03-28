use clap::Parser;
use std::io;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port to connect to
    #[arg(short, long, default_value_t = 9090)]
    port: u16,

    /// Host to connect to
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();
    // Use the library implementation from 'nyx'
    nyx::runtime::execution::nyx_shell_client::run_shell(&args.host, args.port).await
}

use anyhow::Result;
use clap::Parser;

mod cli;
mod db;
mod logs;
mod server;
mod tools;
mod utils;

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let cli = cli::Cli::parse();
    // Run the specified command
    match cli.command {
        cli::Commands::Start {
            endpoint,
            ns,
            db,
            user,
            pass,
            bind_address,
            socket_path,
        } => server::start_server(endpoint, ns, db, user, pass, bind_address, socket_path).await,
    }
}

use crate::server::ServerConfig;
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
            rate_limit_rps,
            rate_limit_burst,
        } => {
            let config = ServerConfig {
                endpoint,
                ns,
                db,
                user,
                pass,
                bind_address,
                socket_path,
                rate_limit_rps,
                rate_limit_burst,
            };
            server::start_server(config).await
        }
    }
}

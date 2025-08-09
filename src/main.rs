use crate::server::ServerConfig;
use anyhow::Result;
use clap::Parser;

mod cli;
mod cloud;
mod db;
mod engine;
mod logs;
mod prompts;
mod resources;
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
            server_url,
            bind_address,
            socket_path,
            auth_disabled,
            rate_limit_rps,
            rate_limit_burst,
            auth_server,
            auth_audience,
            jwe_decryption_key,
        } => {
            // Create the server config
            let config = ServerConfig {
                endpoint,
                ns,
                db,
                user,
                pass,
                server_url,
                bind_address,
                socket_path,
                auth_disabled,
                rate_limit_rps,
                rate_limit_burst,
                auth_server,
                auth_audience,
                jwe_decryption_key,
            };
            server::start_server(config).await
        }
    }
}

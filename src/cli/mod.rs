use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "surrealmcp")]
#[command(about = "SurrealDB MCP Server")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the MCP server
    Start {
        /// The SurrealDB endpoint URL to connect to
        #[arg(short, long, env = "SURREALDB_URL")]
        endpoint: Option<String>,
        /// The SurrealDB namespace to use
        #[arg(long, env = "SURREALDB_NS")]
        ns: Option<String>,
        /// The SurrealDB database to use
        #[arg(long, env = "SURREALDB_DB")]
        db: Option<String>,
        /// The SurrealDB username to use
        #[arg(short, long, env = "SURREALDB_USER")]
        user: Option<String>,
        /// The SurrealDB password to use
        #[arg(short, long, env = "SURREALDB_PASS")]
        pass: Option<String>,
        /// The MCP server bind address (host:port)
        #[arg(long, env = "SURREAL_MCP_BIND_ADDRESS", group = "server")]
        bind_address: Option<String>,
        /// The MCP server Unix socket path
        #[arg(long, env = "SURREAL_MCP_SOCKET_PATH", group = "server")]
        socket_path: Option<String>,
    },
}

mod markdown;
mod server;
mod session;

use anyhow::Context;
use clap::{Parser, ValueEnum};
use serde::Serialize;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Debug, Clone, Parser)]
#[command(author, version, about = "Markdown live preview server for Neovim")]
pub struct Cli {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long, default_value_t = 0)]
    port: u16,

    #[arg(long, value_enum, default_value_t = MermaidMode::Local)]
    mermaid_mode: MermaidMode,

    #[arg(
        long,
        default_value = "https://cdn.jsdelivr.net/npm/mermaid@11.15.0/dist/mermaid.min.js"
    )]
    mermaid_cdn_url: String,

    #[arg(long, default_value = "info")]
    log_level: String,
}

#[derive(Debug, Clone, Copy, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum MermaidMode {
    Local,
    Cdn,
    None,
    LocalWithCdnFallback,
}

#[derive(Debug, Serialize)]
struct StartupInfo<'a> {
    host: &'a str,
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    init_tracing(&cli.log_level)?;

    let bind_addr = format!("{}:{}", cli.host, cli.port);
    let listener = TcpListener::bind(&bind_addr)
        .await
        .with_context(|| format!("failed to bind {bind_addr}"))?;
    let local_addr = listener
        .local_addr()
        .context("failed to read bound local address")?;
    let port = local_addr.port();

    let app_config = server::AppConfig {
        host: cli.host.clone(),
        port,
        mermaid_mode: cli.mermaid_mode,
        mermaid_cdn_url: cli.mermaid_cdn_url,
    };
    let app = server::app(server::AppState::new(app_config));

    println!(
        "{}",
        serde_json::to_string(&StartupInfo {
            host: &cli.host,
            port,
        })?
    );

    tracing::info!(%bind_addr, %port, "server started");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("server failed")?;

    Ok(())
}

fn init_tracing(level: &str) -> anyhow::Result<()> {
    let filter = EnvFilter::try_new(level)
        .or_else(|_| EnvFilter::try_new("info"))
        .context("failed to initialize log filter")?;
    fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
    Ok(())
}

async fn shutdown_signal() {
    if let Err(err) = tokio::signal::ctrl_c().await {
        tracing::warn!(%err, "failed to listen for shutdown signal");
    }
}

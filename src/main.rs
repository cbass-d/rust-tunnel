use anyhow::Result;
use clap::Parser;
use log::info;
use russh::Preferred;
use std::time::Duration;

mod config;
mod server;

#[derive(Parser)]
struct Args {
    // Port to listen on
    #[arg(short, long)]
    port: Option<u16>,
}

#[tokio::main]
async fn main() -> Result<()> {
    match std::env::var("RUST_LOG") {
        Ok(_) => {}
        Err(_) => {
            std::env::set_var("RUST_LOG", "info");
        }
    };
    env_logger::init();

    let args = Args::parse();

    // Load config file using confy
    let rust_tunnel_conf: config::RustTunnelConfig =
        confy::load("rust-tunnel", Some("rustunnel-conf"))?;
    let server_keys = rust_tunnel::get_server_keys(rust_tunnel_conf.server_keys.as_ref())?;
    info!(
        "Using following server keys: {:?}",
        rust_tunnel_conf.server_keys
    );

    // Port provided through CLI overrides port in config
    let port = match args.port {
        Some(port) => {
            info!("Using port from arguments");
            port
        }
        None => {
            info!("Using port from config file");
            rust_tunnel_conf.port
        }
    };

    // Build russh server config using confy config
    let russh_config = russh::server::Config {
        inactivity_timeout: Some(Duration::from_secs(rust_tunnel_conf.inactivity_timeout)),
        auth_rejection_time: Duration::from_secs(rust_tunnel_conf.rejection_time),
        auth_rejection_time_initial: Some(Duration::from_secs(0)),
        keys: server_keys,
        preferred: Preferred {
            ..Preferred::default()
        },
        ..Default::default()
    };

    // Run server
    let bind_addr = format!("0.0.0.0:{port}");
    server::run_server(bind_addr, russh_config).await?;

    Ok(())
}

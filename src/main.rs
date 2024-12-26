use anyhow::{anyhow, Result};
use async_trait::async_trait;
use clap::Parser;
use log::{error, info};
use russh::{ChannelId, Preferred};
use rust_tunnel::config::RustTunnelConfig;
use rust_tunnel::SshHandler;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::signal::{self};
use tokio::task::JoinSet;

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
    let rust_tunnel_conf: RustTunnelConfig = confy::load("rust-tunnel", Some("rustunnel-conf"))?;
    match rust_tunnel_conf.routes.is_empty() {
        true => {
            error!("No routes specified");
            println!(
                "Please specifiy routes in config file: {:?}",
                confy::get_configuration_file_path("rust-tunnel", "rusttunnel-conf").unwrap()
            );
        }
        false => {}
    };

    let server_keys = rust_tunnel::get_server_keys(rust_tunnel_conf.server_keys.as_ref())?;
    info!(
        "Using following server keys: {:?}",
        rust_tunnel_conf.server_keys
    );

    // Build russh server config using confy config
    let server_config = russh::server::Config {
        inactivity_timeout: Some(Duration::from_secs(rust_tunnel_conf.inactivity_timeout)),
        auth_rejection_time: Duration::from_secs(rust_tunnel_conf.rejection_time),
        auth_rejection_time_initial: Some(Duration::from_secs(0)),
        keys: server_keys,
        preferred: Preferred {
            ..Preferred::default()
        },
        ..Default::default()
    };
    let server_config = Arc::new(server_config);

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

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    info!("Listening on port: {}", port);

    let mut open_sessions = JoinSet::new();

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => break,
            Ok((s, peer_addr)) = listener.accept() => {
                let server_config = server_config.clone();
                let handler = SshHandler{};
                let session = russh::server::run_stream(server_config, s, handler).await?;
                println!("bbb");
                open_sessions.spawn(session);
            },
        }
    }

    open_sessions.join_all().await;

    Ok(())
}

use anyhow::Result;
use clap::Parser;
use log::{error, info};
use russh::Preferred;
use rust_tunnel::config::RustTunnelConfig;
use rust_tunnel::{Action, ServerState};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::signal::{self};

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
    let ssh_config = russh::server::Config {
        inactivity_timeout: Some(Duration::from_secs(rust_tunnel_conf.inactivity_timeout)),
        auth_rejection_time: Duration::from_secs(rust_tunnel_conf.rejection_time),
        auth_rejection_time_initial: Some(Duration::from_secs(0)),
        keys: server_keys,
        preferred: Preferred {
            ..Preferred::default()
        },
        ..Default::default()
    };

    // Initialize server state
    let mut server_state = ServerState::new(ssh_config);

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    info!("Listening on port: {}", port);

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                info!("Shutting down server");
                break;
            },
            Ok((s, _peer_addr)) = listener.accept() => {
                let _ = server_state.new_session(s).await.map_err(|err| format!("Unable to add new session: {}", err));
            },
            Some(action) = server_state.action_rx.recv() => {
                match action {
                    Action::StoreChannel{ id, channel } => {
                        server_state.store_channel(id, channel);
                    }
                    Action::RemoveSession { id } => {
                        server_state.remove_session(id).unwrap();
                        info!("Session #{id} closed");
                    }
                    Action::RemoveChannel { channel_id, session_id } => {
                        server_state.remove_channel(channel_id);
                        info!("Channel {} closed in Session #{}", channel_id, session_id);
                    }
                }
            },
        }
    }

    Ok(())
}

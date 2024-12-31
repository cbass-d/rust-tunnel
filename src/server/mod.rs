use anyhow::Result;
use log::info;
use russh::server::Config;
use std::sync::Arc;
use tokio::{
    net::TcpListener,
    signal::ctrl_c,
    sync::mpsc::{self},
};

use server_handler::ServerHandler;

mod server_handler;

pub async fn run_server(bind_addr: String, russh_config: Config) -> Result<()> {
    let listener = TcpListener::bind(bind_addr.clone()).await?;
    let config = Arc::new(russh_config);
    let mut next_id = 1;

    info!("Listening at {}", bind_addr);
    loop {
        tokio::select! {
            _ = ctrl_c() => {
                break;
            },
            Ok((stream, addr)) = listener.accept() => {
                let config = config.clone();
                let handler = ServerHandler::default();
                let russh_session = russh::server::run_stream(config, stream, handler).await?;

                println!("new session");


            }
        }
    }

    Ok(())
}
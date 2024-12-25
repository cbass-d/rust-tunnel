use anyhow::Result;
use async_trait::async_trait;
use clap::Parser;
use log::info;
use rand_core::OsRng;
use russh::{
    keys::{self, Certificate, PrivateKey},
    server::{self, Msg, Server as _, Session},
    Channel, ChannelId, CryptoVec, Preferred,
};
use rust_tunnel::config::RustTunnelConfig;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

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

    let server_config = server::Config {
        inactivity_timeout: Some(rust_tunnel_conf.inactivity_timeout),
        auth_rejection_time: rust_tunnel_conf.rejection_time,
        auth_rejection_time_initial: Some(Duration::from_secs(0)),
        keys: rust_tunnel_conf
            .server_keys
            .iter()
            .map(|key_bytes| PrivateKey::from_bytes(key_bytes).unwrap())
            .collect(),
        preferred: Preferred {
            ..Preferred::default()
        },
        ..Default::default()
    };
    let server_config = Arc::new(server_config);

    let mut sh = Server {
        clients: Arc::new(Mutex::new(HashMap::new())),
        id: 0,
    };

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

    info!("Listening on port: {}", port);

    sh.run_on_address(server_config, ("0.0.0.0", port))
        .await
        .unwrap();

    Ok(())
}

#[derive(Clone)]
struct Server {
    clients: Arc<Mutex<HashMap<usize, (ChannelId, russh::server::Handle)>>>,
    id: usize,
}

impl Server {
    async fn post(&mut self, data: CryptoVec) {
        let mut clients = self.clients.lock().await;
        for (id, (channel, ref mut s)) in clients.iter_mut() {
            if *id != self.id {
                let _ = s.data(*channel, data.clone()).await;
            }
        }
    }
}

impl russh::server::Server for Server {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        let s = self.clone();
        self.id += 1;
        s
    }
    fn handle_session_error(&mut self, _error: <Self::Handler as russh::server::Handler>::Error) {
        eprintln!("Session error: {:#?}", _error);
    }
}

#[async_trait]
impl russh::server::Handler for Server {
    type Error = russh::Error;

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        session: &mut Session,
    ) -> Result<bool, Self::Error> {
        {
            let mut clients = self.clients.lock().await;
            clients.insert(self.id, (channel.id(), session.handle()));
        }

        Ok(true)
    }

    async fn auth_publickey(
        &mut self,
        _: &str,
        _key: &keys::ssh_key::PublicKey,
    ) -> Result<russh::server::Auth, Self::Error> {
        Ok(server::Auth::Accept)
    }

    async fn auth_openssh_certificate(
        &mut self,
        _user: &str,
        _cert: &Certificate,
    ) -> Result<server::Auth, Self::Error> {
        Ok(server::Auth::Accept)
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if data == [3] {
            return Err(russh::Error::Disconnect);
        }

        let data = CryptoVec::from(format!("Got data: {}\r\n", String::from_utf8_lossy(data)));
        self.post(data.clone()).await;
        session.data(channel, data)?;
        Ok(())
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let id = self.id;
        let clients = self.clients.clone();
        tokio::spawn(async move {
            let mut clients = clients.lock().await;
            clients.remove(&id);
        });
    }
}

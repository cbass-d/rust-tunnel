use anyhow::Result;
use async_trait::async_trait;
use rand_core::OsRng;
use russh::keys::{Algorithm, PrivateKey};
use russh::{
    keys::PublicKey,
    server::{Auth, Msg, Session},
    Channel, ChannelId,
};
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{self};

pub mod config;

pub enum Action {
    RemoveSession { id: u64 },
}

pub struct ServerState {
    next_id: u64,
    handles: HashMap<u64, russh::server::Handle>,
    ssh_config: Arc<russh::server::Config>,
    pub action_rx: mpsc::UnboundedReceiver<Action>,
    action_tx: mpsc::UnboundedSender<Action>,
}

impl ServerState {
    pub fn new(ssh_config: russh::server::Config) -> ServerState {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let ssh_config = Arc::new(ssh_config);

        ServerState {
            next_id: 1,
            handles: HashMap::new(),
            ssh_config,
            action_rx,
            action_tx,
        }
    }

    pub async fn new_session(&mut self, stream: TcpStream) -> Result<()> {
        let config = self.ssh_config.clone();
        let id = self.next_id;
        let handler = SshHandler::new(id, self.action_tx.clone());
        let session = russh::server::run_stream(config, stream, handler).await?;
        self.handles.insert(id, session.handle());
        self.next_id += 1;
        println!("New session added");
        Ok(())
    }

    pub fn remove_session(&mut self, id: u64) -> Result<()> {
        self.handles.remove(&id);

        println!("Session #{} removied", id);

        Ok(())
    }
}

pub struct SshHandler {
    id: u64,
    action_tx: mpsc::UnboundedSender<Action>,
}

impl SshHandler {
    pub fn new(id: u64, action_tx: mpsc::UnboundedSender<Action>) -> SshHandler {
        SshHandler { id, action_tx }
    }
}

#[async_trait]
impl russh::server::Handler for SshHandler {
    type Error = russh::Error;

    async fn auth_succeeded(&mut self, _session: &mut Session) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn auth_publickey(&mut self, _user: &str, _key: &PublicKey) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        session: &mut Session,
    ) -> Result<bool, Self::Error> {
        let handle = session.handle();
        Ok(true)
    }

    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let handle = session.handle();
        handle.close(channel).await.unwrap();
        Ok(())
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

        let handle = session.handle();
        handle.data(channel, data.into()).await.unwrap();

        Ok(())
    }
}

impl Drop for SshHandler {
    fn drop(&mut self) {
        let id = self.id;
        self.action_tx.send(Action::RemoveSession { id }).unwrap();
        println!("dropped");
    }
}

// Only supports OpenSSH PEM files currently
pub fn get_server_keys(paths: &Vec<String>) -> Result<Vec<PrivateKey>> {
    // Case where default config is used
    // (No server key provided)
    if paths.is_empty() {
        return Ok(vec![
            PrivateKey::random(&mut OsRng, Algorithm::Ed25519).unwrap()
        ]);
    }

    let mut keys: Vec<PrivateKey> = Vec::new();

    for path in paths {
        let path = OsString::from(path);
        let data = fs::read(path)?;
        let key = PrivateKey::from_openssh(data)?;
        keys.push(key);
    }

    Ok(keys)
}

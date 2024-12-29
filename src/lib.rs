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
use tokio::task::{AbortHandle, JoinSet};

pub mod config;

pub enum Action {
    StoreChannel {
        id: ChannelId,
        channel: Channel<Msg>,
    },
    RemoveSession {
        id: u64,
    },
    RemoveChannel {
        channel_id: ChannelId,
        session_id: u64,
    },
}

pub struct ServerState {
    next_id: u64,
    channels: HashMap<ChannelId, Channel<Msg>>,
    sessions: HashMap<u64, AbortHandle>,
    ssh_config: Arc<russh::server::Config>,
    pub action_rx: mpsc::UnboundedReceiver<Action>,
    action_tx: mpsc::UnboundedSender<Action>,
    session_tasks: JoinSet<Result<(), russh::Error>>,
}

impl ServerState {
    pub fn new(ssh_config: russh::server::Config) -> ServerState {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let ssh_config = Arc::new(ssh_config);

        ServerState {
            next_id: 1,
            channels: HashMap::new(),
            sessions: HashMap::new(),
            ssh_config,
            action_rx,
            action_tx,
            session_tasks: JoinSet::new(),
        }
    }

    pub async fn new_session(&mut self, stream: TcpStream) -> Result<()> {
        let config = self.ssh_config.clone();
        let id = self.next_id;
        let handler = SshSession::new(id, self.action_tx.clone());
        let session = russh::server::run_stream(config, stream, handler).await?;

        let abort_handle = self.session_tasks.spawn(session);
        self.sessions.insert(id, abort_handle);
        self.next_id += 1;
        Ok(())
    }

    pub fn store_channel(&mut self, id: ChannelId, channel: Channel<Msg>) {
        self.channels.insert(id, channel);
    }

    pub fn get_channel(&mut self, id: ChannelId) -> Option<Channel<Msg>> {
        self.channels.remove(&id)
    }

    pub fn remove_session(&mut self, id: u64) -> Result<()> {
        let abort_handle = self
            .sessions
            .get(&id)
            .expect("Abort handle not found for session");

        abort_handle.abort();

        Ok(())
    }

    pub fn remove_channel(&mut self, channel_id: ChannelId) {
        self.channels.remove(&channel_id);
    }
}

pub struct SshSession {
    id: u64,
    action_tx: mpsc::UnboundedSender<Action>,
}

impl SshSession {
    pub fn new(id: u64, action_tx: mpsc::UnboundedSender<Action>) -> SshSession {
        SshSession { id, action_tx }
    }
}

#[async_trait]
impl russh::server::Handler for SshSession {
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
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        let id = channel.id();
        let _ = self
            .action_tx
            .send(Action::StoreChannel { id, channel })
            .unwrap();
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

    async fn channel_close(
        &mut self,
        channel: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        let channel_id = channel;
        let session_id = self.id;
        let _ = self.action_tx.send(Action::RemoveChannel {
            channel_id,
            session_id,
        });

        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        channel: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        println!("requested: {name}");

        if name == "sftp" {}

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

impl Drop for SshSession {
    fn drop(&mut self) {
        let id = self.id;
        let _ = self.action_tx.send(Action::RemoveSession { id });
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

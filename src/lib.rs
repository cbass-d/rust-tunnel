use anyhow::Result;
use async_trait::async_trait;
use rand_core::OsRng;
use russh::keys::{Algorithm, PrivateKey};
use russh::{
    keys::PublicKey,
    server::{Auth, Handle, Msg, Session},
    Channel, ChannelId,
};
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::net::SocketAddr;

pub mod config;

pub struct SshHandler {}

#[async_trait]
impl russh::server::Handler for SshHandler {
    type Error = russh::Error;

    async fn auth_succeeded(&mut self, session: &mut Session) -> Result<(), Self::Error> {
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
    fn drop(&mut self) {}
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

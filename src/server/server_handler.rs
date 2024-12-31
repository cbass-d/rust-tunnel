use anyhow::Result;
use async_trait::async_trait;
use russh::{
    keys::PublicKey,
    server::{Auth, Msg, Session},
    Channel, ChannelId,
};
use russh_sftp::protocol::{File, Name, StatusCode, Version};
use std::collections::HashMap;

#[derive(Default)]
pub struct ServerHandler {
    channel: Option<Channel<Msg>>,
}

#[async_trait]
impl russh::server::Handler for ServerHandler {
    type Error = russh::Error;

    async fn auth_succeeded(&mut self, _session: &mut Session) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn auth_publickey(
        &mut self,
        _user: &str,
        _publickey: &PublicKey,
    ) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        self.channel.insert(channel);
        Ok(true)
    }

    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn channel_close(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if data == [3] {
            session.close(channel);
        }

        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        channel: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if name == "sftp" {
            let id = channel;
            let channel = self.channel.take().unwrap();
            let sftp = SFTPHandler::default();
            session.channel_success(id);
            russh_sftp::server::run(channel.into_stream(), sftp).await;
        } else {
            session.channel_failure(channel);
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct SFTPHandler {}

#[async_trait]
impl russh_sftp::server::Handler for SFTPHandler {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    async fn init(
        &mut self,
        version: u32,
        extensions: HashMap<String, String>,
    ) -> Result<Version, Self::Error> {
        Ok(Version::new())
    }

    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        Ok(Name {
            id,
            files: vec![File::dummy("/")],
        })
    }
}

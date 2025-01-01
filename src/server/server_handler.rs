use anyhow::Result;
use async_trait::async_trait;
use log::info;
use russh::{
    keys::PublicKey,
    server::{Auth, Msg, Session},
    Channel, ChannelId,
};
use russh_sftp::protocol::{File, FileAttributes, Handle, Name, Status, StatusCode, Version};
use std::collections::HashMap;
use tokio::fs;

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
        self.channel = Some(channel);
        Ok(true)
    }

    async fn channel_eof(
        &mut self,
        _channel: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn channel_close(
        &mut self,
        _channel: ChannelId,
        _session: &mut Session,
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
            session.close(channel).unwrap();
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
            session.channel_success(id).unwrap();
            russh_sftp::server::run(channel.into_stream(), sftp).await;
        } else {
            session.channel_failure(channel).unwrap();
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct SFTPHandler {
    root_dir_read_done: bool,
}

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
        info!("client requested version {version} with extensions: {extensions:?}");
        Ok(Version::new())
    }

    async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, Self::Error> {
        if let Ok(_) = fs::File::open(&path).await {
            self.root_dir_read_done = false;
            Ok(Handle { id, handle: path })
        } else {
            Err(StatusCode::NoSuchFile)
        }
    }

    async fn close(&mut self, id: u32, _handle: String) -> Result<Status, Self::Error> {
        Ok(Status {
            id,
            status_code: StatusCode::Ok,
            error_message: "Ok".to_string(),
            language_tag: "en-US".to_string(),
        })
    }

    async fn readdir(&mut self, id: u32, handle: String) -> Result<Name, Self::Error> {
        if !self.root_dir_read_done {
            if let Ok(mut entries) = fs::read_dir(handle).await {
                let mut files = Vec::new();
                while let Some(entry) = entries.next_entry().await.unwrap() {
                    let metadata = entry.metadata().await.unwrap();
                    files.push(File {
                        filename: entry.file_name().to_string_lossy().into_owned(),
                        longname: format!("{:?}", metadata.permissions()),
                        attrs: FileAttributes::default(),
                    });
                }
                self.root_dir_read_done = true;

                return Ok(Name { id, files });
            }
        }

        Err(StatusCode::Eof)
    }

    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        let canonical = fs::canonicalize(&path).await.unwrap();
        Ok(Name {
            id,
            files: vec![File {
                filename: canonical.to_string_lossy().to_string(),
                longname: String::new(),
                attrs: FileAttributes::default(),
            }],
        })
    }
}

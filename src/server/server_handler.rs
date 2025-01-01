use anyhow::Result;
use async_trait::async_trait;
use log::info;
use russh::{
    keys::PublicKey,
    server::{Auth, Msg, Session},
    Channel, ChannelId,
};
use russh_sftp::protocol::{
    Attrs, Data, File, FileAttributes, Handle, Name, OpenFlags, Status, StatusCode, Version,
};
use std::fs::Metadata;
use std::{collections::HashMap, io::SeekFrom};
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncSeekExt},
};

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
    dir_read_done: bool,
    file_read_done: bool,
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
            self.dir_read_done = false;
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
        if !self.dir_read_done {
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
                self.dir_read_done = true;

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

    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        if let Ok(file) = fs::File::open(&path).await {
            let metadata = file.metadata().await.unwrap();
            Ok(Attrs {
                id,
                attrs: FileAttributes::from(&metadata),
            })
        } else {
            Err(StatusCode::NoSuchFile)
        }
    }

    async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        if let Ok(file) = fs::File::open(&path).await {
            let metadata = file.metadata().await.unwrap();
            let fileattrs = FileAttributes::from(&metadata);
            Ok(Attrs {
                id,
                attrs: FileAttributes::from(&metadata),
            })
        } else {
            Err(StatusCode::NoSuchFile)
        }
    }

    async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, Self::Error> {
        if let Ok(file) = fs::File::open(&handle).await {
            let metadata = file.metadata().await.unwrap();
            Ok(Attrs {
                id,
                attrs: FileAttributes::from(&metadata),
            })
        } else {
            Err(StatusCode::NoSuchFile)
        }
    }

    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        len: u32,
    ) -> Result<Data, Self::Error> {
        if !self.file_read_done {
            if let Ok(mut file) = fs::File::open(&handle).await {
                let mut buf: Vec<u8> = vec![0; len as usize];
                let n = file.read(&mut buf).await.unwrap();
                self.file_read_done = true;

                return Ok(Data {
                    id,
                    data: buf[..n].to_vec(),
                });
            } else {
                return Err(StatusCode::NoSuchFile);
            }
        }

        Err(StatusCode::Eof)
    }

    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, Self::Error> {
        match fs::write(handle, data).await {
            Ok(()) => Ok(Status {
                id,
                status_code: StatusCode::Ok,
                error_message: "Ok".to_string(),
                language_tag: "en-US".to_string(),
            }),
            Err(_) => Err(StatusCode::Failure),
        }
    }

    async fn remove(&mut self, id: u32, handle: String) -> Result<Status, Self::Error> {
        match fs::remove_file(&handle).await {
            Ok(()) => Ok(Status {
                id,
                status_code: StatusCode::Ok,
                error_message: "Ok".to_string(),
                language_tag: "en-US".to_string(),
            }),
            Err(_) => Err(StatusCode::Failure),
        }
    }

    async fn rmdir(&mut self, id: u32, path: String) -> Result<Status, Self::Error> {
        match fs::remove_dir(&path).await {
            Ok(()) => Ok(Status {
                id,
                status_code: StatusCode::Ok,
                error_message: "Ok".to_string(),
                language_tag: "en-US".to_string(),
            }),
            Err(e) => Ok(Status {
                id,
                status_code: StatusCode::Failure,
                error_message: e.to_string(),
                language_tag: "en-US".to_string(),
            }),
        }
    }

    async fn mkdir(
        &mut self,
        id: u32,
        path: String,
        attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        match fs::create_dir(&path).await {
            Ok(()) => Ok(Status {
                id,
                status_code: StatusCode::Ok,
                error_message: "Ok".to_string(),
                language_tag: "en-US".to_string(),
            }),
            Err(e) => Ok(Status {
                id,
                status_code: StatusCode::Failure,
                error_message: e.to_string(),
                language_tag: "en-US".to_string(),
            }),
        }
    }

    async fn open(
        &mut self,
        id: u32,
        filename: String,
        pflags: OpenFlags,
        attrs: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        if let Ok(_) = fs::try_exists(&filename).await {
            let file = self.file_read_done = false;
            Ok(Handle {
                id,
                handle: filename,
            })
        } else {
            Err(StatusCode::NoSuchFile)
        }
    }
}

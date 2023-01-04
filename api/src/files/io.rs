use std::{
    any::Any,
    fmt::Debug,
    future::Future,
    io::{self, Cursor},
    path::{Path, PathBuf},
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
};

use reqwest::Client;
use serde::de::DeserializeOwned;
use tokio::fs::{self, create_dir_all};
use tracing::{info_span, instrument, trace, Instrument};
use url::Url;
use zip::ZipArchive;

use crate::{
    metadata::{assets::AssetIndex, game::VersionInfo},
    tasks::{GenerateTask, Handle},
};

use super::{ContentType, Dirs, Source};

type PinBoxFut<R> = Pin<Box<dyn Future<Output = R> + Send + Sync + 'static>>;
type OwnedZipArchive = ZipArchive<Cursor<Vec<u8>>>;

#[derive(Debug, Copy, Clone, Default)]
pub enum Validation {
    NoneAtAll,
    Force,
    #[default]
    Usual,
}

// TODO : try to generify w/ lifetime for source, not to cloning some data
// Currently impossible, because Manager::new_task awaits M: 'static
#[derive(Debug)]
pub struct SyncTask {
    client: Client,
    progress: AtomicU64,

    url: Url,
    path: PathBuf,
    validation: Validation,
    r#type: ContentType,
    size: Option<u64>,
}

impl SyncTask {
    pub fn new(source: Source<'_>, dirs: &Dirs) -> Self {
        Self {
            path: source.local_path(dirs),
            size: source.size,
            r#type: source.r#type,
            url: source.url.into_owned(),

            client: Client::new(),
            progress: AtomicU64::new(0),
            validation: Validation::Usual,
        }
    }

    pub fn with_client(self, client: Client) -> Self {
        Self { client, ..self }
    }

    pub fn with_validation(self, validation: Validation) -> Self {
        Self { validation, ..self }
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn progress(&self) -> u64 {
        self.progress.load(Ordering::Relaxed)
    }

    pub fn size(&self) -> Option<u64> {
        self.size
    }

    #[instrument]
    async fn is_valid(&self) -> io::Result<bool> {
        match self.validation {
            Validation::NoneAtAll => Ok(true),
            Validation::Force => Ok(false),
            Validation::Usual => match fs::metadata(&self.path).await {
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
                Ok(_) if self.size.is_none() => Ok(true),
                Ok(metadata) => Ok(metadata.len() == self.size.unwrap()),
                Err(e) => Err(e),
            },
        }
    }

    #[instrument]
    async fn download(&self) -> io::Result<Vec<u8>> {
        let mut response = self
            .client
            .get(self.url.clone())
            .send()
            .instrument(info_span!("wait_for_response"))
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        match (self.size, response.content_length()) {
            (Some(source_len), Some(content_len)) if source_len != content_len => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "source and content sizes mismatch",
                ));
            }
            _ => (),
        }

        let buf = async {
            let buf_size = self.size.or(response.content_length()).unwrap_or_default();
            let mut buf = Vec::with_capacity(buf_size as usize);
            trace!(buf_size, "allocated buf");
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
            {
                let len = chunk.len();
                trace!(len, "new chunk arrived");
                buf.extend_from_slice(chunk.as_ref());
                self.progress.fetch_add(len as u64, Ordering::Relaxed);
            }

            io::Result::Ok(buf)
        }
        .instrument(info_span!("fetch_data"))
        .await?;

        Ok(buf)
    }

    #[instrument]
    async fn read_local(&self) -> io::Result<Vec<u8>> {
        fs::read(&self.path).await
    }

    #[instrument(skip(buf))]
    fn deserialize_json<T: DeserializeOwned>(&self, buf: &[u8]) -> io::Result<T> {
        serde_json::from_slice(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    #[instrument(skip(buf))]
    async fn write_to_file(&self, buf: &[u8]) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            create_dir_all(parent).await?;
        }
        fs::write(&self.path, buf).await
    }

    #[instrument(skip(buf))]
    fn read_zip(&self, buf: Vec<u8>) -> io::Result<OwnedZipArchive> {
        // TODO : error
        ZipArchive::new(Cursor::new(buf)).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}

impl GenerateTask for SyncTask {
    type Output = io::Result<Box<dyn Any + Send + Sync + 'static>>;
    type Future = PinBoxFut<Self::Output>;

    fn task(handle: Handle<Self, Self::Output>) -> Self::Future {
        Box::pin(
            async move {
                let is_valid = handle.is_valid().await?;
                if let ty @ (ContentType::AssetIndex
                | ContentType::VersionInfo
                | ContentType::NativeLibrary) = handle.r#type
                {
                    let bytes = if is_valid {
                        handle.read_local().await?
                    } else {
                        let buf = handle.download().await?;
                        handle.write_to_file(&buf).await?;
                        buf
                    };

                    match ty {
                        ContentType::AssetIndex => Self::Output::Ok(Box::new(
                            handle.deserialize_json::<AssetIndex>(&bytes)?,
                        )),
                        ContentType::VersionInfo => Self::Output::Ok(Box::new(
                            handle.deserialize_json::<VersionInfo>(&bytes)?,
                        )),
                        ContentType::NativeLibrary => {
                            Self::Output::Ok(Box::new(handle.read_zip(bytes)?))
                        }
                        _ => unreachable!(),
                    }
                } else {
                    if !is_valid {
                        let buf = handle.download().await?;
                        handle.write_to_file(&buf).await?;
                    }
                    Self::Output::Ok(Box::new(()))
                }
            }
            .in_current_span(),
        )
    }
}

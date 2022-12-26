use std::{
    fmt::Debug,
    io,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use reqwest::Client;
use serde::de::DeserializeOwned;
use tokio::fs::{self, create_dir_all};
use tracing::{debug, info_span, instrument, trace, Instrument};
use url::Url;

use super::{Dirs, Source};

#[derive(Debug)]
pub struct DownloadItem {
    url: Url,
    path: Option<PathBuf>,
    count: AtomicU64,
    size: Option<u64>,
}

impl DownloadItem {
    pub fn new(value: Source<'_>, dirs: &Dirs) -> Self {
        Self {
            path: value.local_path(dirs),
            url: value.url.into_owned(),
            count: Default::default(),
            size: value.size,
        }
    }

    #[instrument]
    pub async fn validate_local(&self) -> io::Result<bool> {
        match self.path.as_ref() {
            Some(path) => match fs::metadata(path).await {
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
                Ok(_) if self.size.is_none() => Ok(true),
                Ok(metadata) => Ok(metadata.len() == self.size.unwrap()),
                Err(e) => Err(e),
            },
            _ => Ok(true),
        }
    }

    #[instrument]
    pub async fn download(&self, client: &Client) -> io::Result<Vec<u8>> {
        let mut response = client
            .get(self.url.clone())
            .send()
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        debug!(?response, "got response");

        match (self.size, response.content_length()) {
            (Some(source_len), Some(content_len)) if source_len != content_len => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "source and content sizes mismatch",
                ));
            }
            _ => (),
        }

        let buf_size = self.size.or(response.content_length()).unwrap_or_default();
        let mut buf = Vec::with_capacity(buf_size as usize);
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
        {
            let len = chunk.len();
            trace!(len, "new chunk arrived");
            buf.extend_from_slice(chunk.as_ref());
            self.count.fetch_add(len as u64, Ordering::Relaxed);
        }

        if let Some(path) = self.path.as_ref() {
            async {
                if let Some(parent) = path.parent() {
                    create_dir_all(parent).await?;
                }
                fs::write(path, &buf).await?;

                io::Result::Ok(())
            }
            .instrument(info_span!("write_to_file"))
            .await?;
        }

        Ok(buf)
    }

    #[instrument]
    pub async fn download_json<T: DeserializeOwned>(&self, client: &Client) -> io::Result<T> {
        let buf = self.download(client).await?;

        info_span!("deserialize_json").in_scope(|| {
            serde_json::from_slice(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        })
    }
}

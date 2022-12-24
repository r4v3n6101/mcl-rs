use std::{
    fmt::{self, Display},
    io,
    path::Path,
};

use reqwest::IntoUrl;
use tokio::{
    fs::{create_dir_all, File},
    io::{AsyncWriteExt, BufWriter},
};
use tracing::{debug, instrument, trace};
use url::Url;

use super::{FutureTask, Handle, Value};

#[derive(Debug)]
pub struct DownloadMetadata {
    url: Url,
    path: Box<Path>,

    downloaded_bytes: u64,
    content_size: Option<u64>,
}

impl Display for DownloadMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.url, self.path.display())
    }
}

impl DownloadMetadata {
    pub fn current_progress(&self) -> Option<u64> {
        Some(self.downloaded_bytes)
    }

    pub fn max_progress(&self) -> Option<u64> {
        self.content_size
    }
}

impl DownloadMetadata {
    pub fn new<U, P>(url: U, path: P) -> Self
    where
        U: IntoUrl,
        P: Into<Box<Path>>,
    {
        DownloadMetadata {
            url: url.into_url().unwrap(),
            path: path.into(),
            downloaded_bytes: Default::default(),
            content_size: Default::default(),
        }
    }
}

#[instrument]
pub async fn download_file(handle: Handle) -> io::Result<()> {
    const BUF_SIZE: usize = 1024 * 16; //  16kb

    let mut response = {
        let response = reqwest::get(handle.metadata::<DownloadMetadata>().url.clone())
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        debug!(?response, "Remote responded");
        handle.metadata_mut::<DownloadMetadata>().content_size = response.content_length();

        response
    };

    let mut output = {
        let path = &handle.metadata::<DownloadMetadata>().path;
        if let Some(parent) = path.parent() {
            create_dir_all(parent).await?;
        }
        let file = File::create(path).await?;
        debug!(?file, "File created");

        BufWriter::with_capacity(BUF_SIZE, file)
    };

    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
    {
        let len = chunk.len();
        output.write_all(&chunk).await?;
        trace!(len, "New chunk written");

        handle.metadata_mut::<DownloadMetadata>().downloaded_bytes += len as u64;
    }
    output.flush().await?;

    Ok(())
}

pub fn download_file_task(handle: Handle) -> FutureTask {
    Box::pin(async move {
        download_file(handle).await?;

        Ok(Box::new(()) as Value)
    })
}

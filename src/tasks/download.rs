use std::{
    fmt::{self, Display},
    num::NonZeroU64,
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use reqwest::IntoUrl;
use tokio::{
    fs::{create_dir_all, File},
    io::{AsyncWriteExt, BufWriter},
};
use tracing::{debug, trace};
use url::Url;

use super::{display::Progress, Handle, Task};

#[derive(Debug)]
pub struct JobMetadata {
    url: Url,
    path: Box<Path>,

    downloaded_bytes: AtomicU64,
    content_size: AtomicU64,
}

impl Display for JobMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.url, self.path.display())
    }
}

impl Progress for JobMetadata {
    type Output = u64;

    fn progress(&self) -> (Self::Output, Option<Self::Output>) {
        (
            self.downloaded_bytes.load(Ordering::Relaxed),
            NonZeroU64::new(self.content_size.load(Ordering::Relaxed)).map(NonZeroU64::get),
        )
    }
}

impl JobMetadata {
    pub fn new<U, P>(url: U, path: P) -> Self
    where
        U: IntoUrl,
        P: Into<Box<Path>>,
    {
        JobMetadata {
            url: url.into_url().unwrap(),
            path: path.into(),
            downloaded_bytes: Default::default(),
            content_size: Default::default(),
        }
    }
}

pub fn task(handle: Arc<Handle<JobMetadata, ()>>) -> Task<()> {
    const BUF_SIZE: usize = 1024 * 16; //  16kb

    // TODO : span an instrument
    Box::pin(async move {
        let metadata = handle.metadata();

        if let Some(parent) = metadata.path.parent() {
            create_dir_all(parent).await?;
        }
        let file = File::create(&metadata.path).await?;
        let mut output = BufWriter::with_capacity(BUF_SIZE, file);
        let mut response = reqwest::get(metadata.url.clone()).await?;

        metadata.content_size.store(
            response.content_length().unwrap_or_default(),
            Ordering::Relaxed,
        );

        debug!(?response, "Remote responded");
        while let Some(chunk) = response.chunk().await? {
            let len = chunk.len();
            trace!(len, "New chunk arrived");
            output.write_all(&chunk).await?;

            metadata
                .downloaded_bytes
                .fetch_add(len as u64, Ordering::Relaxed);
        }
        output.flush().await?;

        Ok(())
    })
}

use std::io;

use tokio::fs;
use tracing::instrument;

use crate::files::{Dirs, Source};

use super::Handle;

pub struct ValidateMetadata<'a> {
    source: Source<'a>,
    dirs: &'a Dirs,
}

#[instrument]
pub async fn validate(handle: Handle) -> io::Result<bool> {
    let metadata = handle.metadata::<ValidateMetadata>();

    let path = metadata.source.local_path(metadata.dirs);
    let expected_size = metadata.source.size;
    match fs::metadata(&path).await {
        // supposed to be if let to reduce unwrap
        Ok(file_metadata) if expected_size.is_some() => {
            Ok(file_metadata.len() == expected_size.unwrap())
        }
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

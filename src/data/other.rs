use std::{io::Cursor, sync::Arc};

use bytes::Bytes;
use zip::ZipArchive;

use super::RemoteSource;

#[derive(Debug, Clone)]
pub struct JustFile {
    pub data: Bytes,
}

#[derive(Debug, Clone)]
pub struct ZippedFile {
    /// The original source of archive (nested archives aren't supported)
    pub source: RemoteSource,
    /// Archive loaded to memory (only headers, names and reader in memory)
    pub archive: ZipArchive<Cursor<Bytes>>,
    /// Files that should be excluded when extracting
    pub exclude: Arc<[Arc<str>]>,
}

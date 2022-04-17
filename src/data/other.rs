use std::{io::Cursor, sync::Arc};

use bytes::Bytes;
use zip::ZipArchive;

use super::Source;

#[derive(Debug, Clone)]
pub struct JustFile {
    pub data: Bytes,
}

#[derive(Debug, Clone)]
pub struct ZippedFile {
    pub source: Arc<Source>,
    pub archive: ZipArchive<Cursor<Bytes>>,
}

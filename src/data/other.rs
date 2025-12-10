use std::{io::Cursor, ops::Deref, sync::Arc};

use bytes::Bytes;
use stable_deref_trait::StableDeref;
use yoke::{CloneableCart, Yokeable};
use zip::{ZipArchive, result::ZipResult};

#[derive(Debug, Clone)]
pub struct JustFile {
    pub data: Bytes,
}

#[derive(Debug, Clone)]
pub struct ZippedNatives {
    /// Archive loaded to memory (headers + data).
    pub archive: SharedZipArchive,
    /// Files that should not be extracted.
    pub exclude: Arc<[Arc<str>]>,
    // Version of (usually it's version of the game).
    pub classifier: Arc<str>,
}

/// Cow-like for entries.
/// It's [`Yokeable`] in order to being attached to [`ZipArchive`].
#[derive(Yokeable, Debug, Clone)]
pub struct ZipEntry<'a> {
    pub name: &'a str,
    // TODO: zip's fucked code can't provide me entry by immutable ref ://
    // pub size: u64,
    // pub compressed_size: u64,
}

#[derive(Debug, Clone)]
pub struct SharedZipArchive(
    /// Actually it's 2 Arc-s inside. For now.
    ZipArchive<Cursor<Bytes>>,
);

impl SharedZipArchive {
    pub fn new(data: Bytes) -> ZipResult<Self> {
        ZipArchive::new(Cursor::new(data)).map(Self)
    }

    pub fn get_data(&self) -> Bytes {
        self.0.clone().into_inner().into_inner()
    }
}

impl Deref for SharedZipArchive {
    type Target = ZipArchive<Cursor<Bytes>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// Safety: it's shared
unsafe impl CloneableCart for SharedZipArchive {}

// Safety: it's shared
unsafe impl StableDeref for SharedZipArchive {}

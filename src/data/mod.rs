use std::{io, sync::Arc};

use bytes::Bytes;
use url::Url;

use crate::data::{mojang::Sha1Hash, other::ZippedFile};

pub mod config;
pub mod mojang;
pub mod other;

mod imp;

/// Something meaningful and physically existing like a JSON with data
/// or file, or even an archive with files.
pub trait Artifact: GetBytes + 'static {
    /// It's a reference semantically.
    type Config<'this>
    where
        Self: 'this;

    /// Artifacts provided by [`Self`] in form of [`Source`],
    /// so they aren't resolved to concrete type.
    fn provides<'this>(
        &'this self,
        config: Self::Config<'this>,
    ) -> impl Iterator<Item = Source> + 'this;
}

///  Those can be serialized into bytes.
pub trait GetBytes {
    /// Get or calculate bytes for the object.
    fn calc_bytes(&self) -> io::Result<Bytes>;
}

#[derive(Debug, Clone)]
pub struct RemoteSource {
    pub url: Arc<Url>,
    pub name: Arc<str>,
    pub kind: SourceKind,
    pub hash: Option<Sha1Hash>,
    pub size: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ArchivedSource {
    pub zipped: ZippedFile,
    pub index: usize,
}

#[derive(Debug, Clone)]
pub enum Source {
    Remote(RemoteSource),
    Archive(ArchivedSource),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum SourceKind {
    VersionManifest,
    VersionInfo,
    ClientJar,
    ServerJar,
    Library,
    ZippedLibrary {
        exclude: Arc<[Arc<str>]>,
    },
    AssetIndex,
    Asset {
        legacy: bool,
    },
    JvmInfo {
        platform: Arc<str>,
        jvm_mojang_name: Arc<str>,
    },
    JvmFile {
        platform: Arc<str>,
        jvm_mojang_name: Arc<str>,
        executable: bool,
        compressed: bool,
    },
}

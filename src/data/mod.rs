use std::{io, sync::Arc};

use bytes::Bytes;
use url::Url;
use yoke::Yoke;

use crate::data::other::{SharedZipArchive, ZipEntry};

use self::mojang::Sha1Hash;

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

#[derive(Debug)]
pub enum Source {
    Remote {
        url: Arc<Url>,
        name: Arc<str>,
        kind: SourceKind,
        hash: Option<Sha1Hash>,
        size: Option<u64>,
    },
    Archive {
        /// Entry attached to its archive.
        entry: Yoke<ZipEntry<'static>, SharedZipArchive>,
        /// Kind of archive.
        kind: ArchiveKind,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum SourceKind {
    VersionManifest,
    VersionInfo,
    ClientJar,
    ServerJar,
    Library,
    ZippedNatives {
        classifier: Arc<str>,
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

#[derive(Debug, Clone)]
pub enum ArchiveKind {
    Natives {
        /// Classifier for unpacking natives into dir.
        classifier: Arc<str>,
    },
}

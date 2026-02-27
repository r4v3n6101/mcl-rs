use std::{borrow::Cow, sync::Arc};

use url::Url;
use yoke::{Yoke, Yokeable};

use self::{
    mojang::Sha1Hash,
    other::{SharedZipArchive, ZipEntry},
};

pub mod config;
pub mod mojang;
pub mod other;

mod imp;

/// Something meaningful and physically existing like a JSON with data or file, or even an archive with files.
pub trait Artifact {
    type Config;

    fn provides(&self, config: Self::Config) -> impl Iterator<Item = Source<'_>> + '_;
}

#[derive(Yokeable, Debug)]
pub enum Source<'src> {
    Remote {
        url: Arc<Url>,
        name: Cow<'src, str>,
        kind: SourceKind<'src>,
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
pub enum SourceKind<'src> {
    VersionManifest,
    VersionInfo,
    ClientJar,
    ServerJar,
    Library,
    ZippedNatives {
        classifier: Arc<str>,
        exclude: &'src [&'src str],
    },
    AssetIndex,
    Asset {
        legacy: bool,
    },
    JvmInfo {
        platform: &'src str,
        jvm_mojang_name: &'src str,
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

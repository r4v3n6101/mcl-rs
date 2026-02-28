use std::{borrow::Cow, sync::Arc};

use url::Url;
use yoke::{Yoke, Yokeable, erased::ErasedArcCart};

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
    type Config: for<'a> Yokeable<'a>;

    fn provides(
        &self,
        config: Yoke<Self::Config, ErasedArcCart>,
    ) -> impl Iterator<Item = Source<'_>> + '_;
}

#[derive(Yokeable)]
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
        platform: Yoke<&'static str, ErasedArcCart>,
        jvm_mojang_name: Yoke<&'static str, ErasedArcCart>,
        executable: bool,
        compressed: bool,
    },
}

pub enum ArchiveKind {
    Natives {
        /// Classifier for unpacking natives into dir.
        classifier: Arc<str>,
    },
}

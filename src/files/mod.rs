use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use url::Url;

pub mod sources;

#[derive(Debug)]
pub struct Dirs {
    pub root: Cow<'static, Path>,
    pub assets: Cow<'static, Path>,
    pub libraries: Cow<'static, Path>,
    pub natives: Cow<'static, Path>,
    pub version: Cow<'static, Path>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum ContentType {
    AssetIndex,
    LegacyAsset,
    Asset,
    Library,
    NativeLibrary,
    ClientJar,
    VersionInfo,
}

#[derive(Debug)]
pub struct Source<'list> {
    pub url: Cow<'list, Url>,
    pub name: Cow<'list, str>,
    pub r#type: ContentType,

    pub hash: Option<&'list str>,
    pub size: Option<u64>,
}

impl Source<'_> {
    pub fn local_path(&self, dirs: &Dirs) -> PathBuf {
        match self.r#type {
            ContentType::AssetIndex => dirs
                .assets
                .join("indexes")
                .join(format!("{}.json", self.name)),
            ContentType::Asset => dirs.assets.join("objects").join(self.name.as_ref()),
            ContentType::LegacyAsset => dirs.assets.join("legacy").join(self.name.as_ref()),
            ContentType::Library | ContentType::NativeLibrary => dirs.libraries.to_path_buf(),
            ContentType::ClientJar => dirs.version.join(self.name.as_ref()).join("client.jar"),
            ContentType::VersionInfo => dirs.version.join(self.name.as_ref()).join("info.json"),
        }
    }
}

pub trait SourcesList<'a> {
    type Iter: Iterator<Item = Source<'a>>;

    fn sources(self) -> Self::Iter;
}

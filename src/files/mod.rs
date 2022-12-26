use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use url::Url;

pub mod sources;
// TODO : rename
pub mod io;

// I can't create &'static Path, should it be replaces w/ PathBuf-s?
#[derive(Debug)]
pub struct Dirs {
    pub root: Cow<'static, Path>,
    pub assets: Cow<'static, Path>,
    pub libraries: Cow<'static, Path>,
    // TODO : where to store natives? globally?
    pub natives: Cow<'static, Path>,
    pub versions: Cow<'static, Path>,
}

impl Default for Dirs {
    fn default() -> Self {
        let root_dir = dirs::data_dir()
            .map(|p| p.join("minecraft"))
            .or_else(|| dirs::home_dir().map(|p| p.join(".minecraft")))
            .expect("can't get root dir");
        Self {
            root: Cow::Owned(root_dir.clone()),
            assets: Cow::Owned(root_dir.join("assets")),
            libraries: Cow::Owned(root_dir.join("libraries")),
            natives: Cow::Owned(root_dir.join("natives")),
            versions: Cow::Owned(root_dir.join("versions")),
        }
    }
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
    VersionsManifest,
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
    pub fn local_path(&self, dirs: &Dirs) -> Option<PathBuf> {
        match self.r#type {
            ContentType::AssetIndex => {
                Some(dirs.assets.join(format!("indexes/{}.json", self.name)))
            }
            ContentType::Asset => Some(dirs.assets.join("objects").join(self.name.as_ref())),
            ContentType::LegacyAsset => Some(dirs.assets.join("legacy").join(self.name.as_ref())),
            ContentType::Library | ContentType::NativeLibrary => {
                Some(dirs.libraries.join(self.name.as_ref()))
            }
            ContentType::ClientJar => {
                Some(dirs.versions.join(self.name.as_ref()).join("client.jar"))
            }
            ContentType::VersionInfo => {
                Some(dirs.versions.join(self.name.as_ref()).join("info.json"))
            }
            ContentType::VersionsManifest => None,
        }
    }
}

pub trait SourcesList<'a> {
    type Iter: Iterator<Item = Source<'a>>;

    fn sources(self) -> Self::Iter;
}

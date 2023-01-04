use std::{borrow::Cow, path::PathBuf};

use url::Url;

pub mod sources;
// TODO : rename
pub mod io;

#[derive(Debug)]
pub struct Dirs {
    pub root: PathBuf,
    pub assets: PathBuf,
    pub libraries: PathBuf,
    // TODO : where to store natives? globally?
    pub natives: PathBuf,
    pub versions: PathBuf,
}

impl Default for Dirs {
    fn default() -> Self {
        let root_dir = dirs::data_dir()
            .map(|p| p.join("minecraft"))
            .or_else(|| dirs::home_dir().map(|p| p.join(".minecraft")))
            .expect("can't get root dir");
        Self {
            root: root_dir.clone(),
            assets: root_dir.join("assets"),
            libraries: root_dir.join("libraries"),
            natives: root_dir.join("natives"),
            versions: root_dir.join("versions"),
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
            ContentType::AssetIndex => dirs.assets.join(format!("indexes/{}.json", self.name)),
            ContentType::Asset => dirs.assets.join("objects").join(self.name.as_ref()),
            ContentType::LegacyAsset => dirs.assets.join("legacy").join(self.name.as_ref()),
            ContentType::Library | ContentType::NativeLibrary => {
                dirs.libraries.join(self.name.as_ref())
            }
            ContentType::ClientJar => dirs.versions.join(self.name.as_ref()).join("client.jar"),
            ContentType::VersionInfo => dirs.versions.join(self.name.as_ref()).join("info.json"),
        }
    }
}

pub trait SourcesList<'a> {
    type Iter: Iterator<Item = Source<'a>>;

    fn sources(self) -> Self::Iter;
}

use std::path::PathBuf;

use crate::sources::{Source, SourceKind};

#[derive(Debug, Clone)]
pub struct Dirs {
    pub root: PathBuf,
    pub assets: PathBuf,
    pub libraries: PathBuf,
    pub versions: PathBuf,
}

impl Dirs {
    pub fn locate(&self, src: &Source<'_>) -> PathBuf {
        match src.kind {
            SourceKind::AssetIndex => self.assets.join(format!("indexes/{}.json", src.name)),
            SourceKind::Asset => self.assets.join("objects").join(src.name.as_ref()),
            SourceKind::LegacyAsset => self.assets.join("legacy").join(src.name.as_ref()),
            SourceKind::Library | SourceKind::NativeLibrary => {
                self.libraries.join(src.name.as_ref())
            }
            SourceKind::ClientJar => self.versions.join(src.name.as_ref()).join("client.jar"),
            SourceKind::ServerJar => self.versions.join(src.name.as_ref()).join("server.jar"),
            SourceKind::VersionInfo => self.versions.join(src.name.as_ref()).join("info.json"),
        }
    }
}

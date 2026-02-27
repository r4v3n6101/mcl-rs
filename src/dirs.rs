use std::path::PathBuf;

use crate::data::{ArchiveKind, Source, SourceKind};

#[derive(Debug, Clone)]
pub struct Dirs {
    pub root: PathBuf,
    pub assets: PathBuf,
    pub libraries: PathBuf,
    pub versions: PathBuf,
    pub runtime: PathBuf,
}

impl Dirs {
    pub fn locate(&self, src: &Source) -> PathBuf {
        match src {
            Source::Remote { kind, name, .. } => match &kind {
                SourceKind::VersionManifest => build_path(self.root.clone(), [name], None, "json"),
                SourceKind::AssetIndex => {
                    build_path(self.assets.clone(), ["indexes", name], None, "json")
                }
                SourceKind::Asset { legacy: false } => {
                    build_path(self.assets.clone(), ["objects", name], None, None)
                }
                SourceKind::Asset { legacy: true } => {
                    build_path(self.assets.clone(), ["legacy", name], None, None)
                }
                SourceKind::Library | SourceKind::ZippedNatives { .. } => {
                    build_path(self.libraries.clone(), [name], None, None)
                }
                SourceKind::ClientJar => {
                    build_path(self.versions.clone(), [name, name], None, "jar")
                }
                SourceKind::ServerJar => {
                    build_path(self.versions.clone(), [name, name], "_server", "jar")
                }
                SourceKind::VersionInfo => {
                    build_path(self.versions.clone(), [name, name], None, "json")
                }
                SourceKind::JvmInfo {
                    platform,
                    jvm_mojang_name,
                } => build_path(
                    self.runtime.clone(),
                    [jvm_mojang_name, platform, jvm_mojang_name, &**name],
                    "_info",
                    "json",
                ),
                SourceKind::JvmFile {
                    platform,
                    jvm_mojang_name,
                    ..
                } => build_path(
                    self.runtime.clone(),
                    [jvm_mojang_name, platform, jvm_mojang_name, &**name],
                    None,
                    None,
                ),
            },
            Source::Archive {
                entry,
                kind: ArchiveKind::Natives { classifier },
            } => build_path(
                self.versions.clone(),
                [classifier, "natives", entry.get().name],
                None,
                None,
            ),
        }
    }
}

fn build_path<'a>(
    mut path_buf: PathBuf,
    paths: impl IntoIterator<Item = impl AsRef<str>>,
    suffix: impl Into<Option<&'a str>>,
    extension: impl Into<Option<&'a str>>,
) -> PathBuf {
    paths.into_iter().for_each(|p| path_buf.push(p.as_ref()));
    if let Some(suffix) = suffix.into() {
        path_buf.as_mut_os_string().push(suffix);
    }
    if let Some(extension) = extension.into() {
        path_buf.add_extension(extension);
    }
    path_buf
}

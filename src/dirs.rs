use std::path::PathBuf;

use crate::data::{Source, SourceKind};

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
            Source::Remote { name, kind, .. } => match &kind {
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
                SourceKind::Library { .. } => {
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
                    [jvm_mojang_name, platform, jvm_mojang_name, name],
                    "_info",
                    "json",
                ),
                SourceKind::JvmFile {
                    platform,
                    jvm_mojang_name,
                    ..
                } => build_path(
                    self.runtime.clone(),
                    [jvm_mojang_name, platform, jvm_mojang_name, name],
                    None,
                    None,
                ),
            },
            Source::Archive { zipped, index } => {
                let src @ Source::Remote { .. } = &*zipped.source else {
                    // TODO
                    panic!("nested archives not supported");
                };

                let path = self.locate(src);
                let name = zipped
                    .archive
                    .name_for_index(*index)
                    .map(ToString::to_string)
                    .unwrap_or(format!("unnamed{index}"));

                path.parent().unwrap().join(name)
            }
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

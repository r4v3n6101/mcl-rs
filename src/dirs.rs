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
            SourceKind::AssetIndex => build_path(
                self.assets.clone(),
                ["indexes", src.name.as_ref()],
                None,
                "json",
            ),
            SourceKind::Asset => build_path(
                self.assets.clone(),
                ["objects", src.name.as_ref()],
                None,
                None,
            ),
            SourceKind::LegacyAsset => build_path(
                self.assets.clone(),
                ["legacy", src.name.as_ref()],
                None,
                None,
            ),
            SourceKind::Library | SourceKind::NativeLibrary => {
                build_path(self.libraries.clone(), [src.name.as_ref()], None, None)
            }
            SourceKind::ClientJar => build_path(
                self.versions.clone(),
                [src.name.as_ref(), src.name.as_ref()],
                None,
                "jar",
            ),
            SourceKind::ServerJar => build_path(
                self.versions.clone(),
                [src.name.as_ref(), src.name.as_ref()],
                "_server",
                "jar",
            ),
            SourceKind::VersionInfo => build_path(
                self.versions.clone(),
                [src.name.as_ref(), src.name.as_ref()],
                None,
                "json",
            ),
        }
    }
}

#[inline(always)]
fn build_path<'a>(
    mut path_buf: PathBuf,
    paths: impl IntoIterator<Item = &'a str>,
    suffix: impl Into<Option<&'a str>>,
    extension: impl Into<Option<&'a str>>,
) -> PathBuf {
    paths.into_iter().for_each(|p| path_buf.push(p));
    if let Some(suffix) = suffix.into() {
        path_buf.as_mut_os_string().push(suffix);
    }
    if let Some(extension) = extension.into() {
        path_buf.add_extension(extension);
    }
    path_buf
}

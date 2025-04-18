use std::{borrow::Cow, collections::BTreeMap, iter};

use url::Url;

use crate::{
    metadata::{
        AssetIndex, AssetMetadata, JvmContent, JvmInfo, JvmManifest, JvmPlatform, JvmResource,
        LibraryResource, Resource, Sha1Hash, Version, VersionInfo,
    },
    util,
};

#[derive(Debug)]
pub struct Source<'list> {
    pub url: Cow<'list, Url>,
    pub name: Cow<'list, str>,
    pub kind: SourceKind<'list>,
    pub hash: Option<&'list Sha1Hash>,
    pub size: Option<u64>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
#[non_exhaustive]
pub enum SourceKind<'src> {
    AssetIndex,
    Asset {
        legacy: bool,
    },
    Library,
    NativeLibrary,
    ClientJar,
    ServerJar,
    VersionInfo,
    JvmInfo {
        platform: &'src str,
        jvm_mojang_name: &'src str,
    },
    JvmFile {
        platform: &'src str,
        jvm_mojang_name: &'src str,
        executable: bool,
        compressed: bool,
    },
}

pub trait SourceList<'a> {
    fn sources(self) -> impl Iterator<Item = Source<'a>> + 'a;
}

impl<'version> SourceList<'version> for &'version Version {
    fn sources(self) -> impl Iterator<Item = Source<'version>> + 'version {
        iter::once(Source {
            kind: SourceKind::VersionInfo,
            url: Cow::Borrowed(&self.url),
            name: Cow::Borrowed(&self.id),
            hash: None,
            size: None,
        })
    }
}

pub struct AssetList<'index, 'origin> {
    index: &'index AssetIndex,
    origin: &'origin Url,
}

impl<'index, 'origin> AssetList<'index, 'origin> {
    pub fn new(index: &'index AssetIndex, origin: &'origin Url) -> Self {
        Self { index, origin }
    }
}

impl<'index, 'origin: 'index> SourceList<'index> for AssetList<'index, 'origin> {
    fn sources(self) -> impl Iterator<Item = Source<'index>> + 'index {
        let Self { origin, index } = self;

        index
            .objects
            .iter()
            .map(move |(path, AssetMetadata { hash, size })| {
                let hash_path = {
                    let hash = hash.to_string();
                    format!("{}/{}", &hash[..2], &hash)
                };
                Source {
                    kind: SourceKind::Asset {
                        legacy: index.map_to_resources,
                    },
                    url: Cow::Owned(
                        origin
                            .join(&hash_path)
                            .expect("couldn't create url with hash"),
                    ),
                    name: if index.map_to_resources {
                        Cow::Borrowed(path)
                    } else {
                        Cow::Owned(hash_path)
                    },
                    hash: Some(hash),
                    size: Some(*size),
                }
            })
    }
}

pub struct ArtifactList<'info, 'params> {
    info: &'info VersionInfo,
    params: &'params BTreeMap<&'params str, bool>,
}

impl<'info, 'params> ArtifactList<'info, 'params> {
    pub fn new(info: &'info VersionInfo, params: &'params BTreeMap<&'params str, bool>) -> Self {
        Self { info, params }
    }
}

impl<'info, 'params: 'info> SourceList<'info> for ArtifactList<'info, 'params> {
    fn sources(self) -> impl Iterator<Item = Source<'info>> + 'info {
        let Self { info, params } = self;

        let asset_index = iter::once(Source {
            kind: SourceKind::AssetIndex,
            url: Cow::Borrowed(&info.asset_index.resource.url),
            name: Cow::Borrowed(&info.asset_index.id),
            hash: Some(&info.asset_index.resource.hash),
            size: Some(info.asset_index.resource.size),
        });

        let client_jar = iter::once(Source {
            kind: SourceKind::ClientJar,
            url: Cow::Borrowed(&info.downloads.client.url),
            name: Cow::Borrowed(&info.id),
            hash: Some(&info.downloads.client.hash),
            size: Some(info.downloads.client.size),
        });

        let server_jar = info
            .downloads
            .server
            .as_ref()
            .map(|Resource { hash, size, url }| Source {
                kind: SourceKind::ServerJar,
                url: Cow::Borrowed(url),
                name: Cow::Borrowed(&info.id),
                hash: Some(hash),
                size: Some(*size),
            });

        let libraries = info
            .libraries
            .iter()
            .filter(|lib| lib.rules.is_allowed(params))
            .flat_map(|lib| {
                let library = lib.resources.artifact.as_ref().map(
                    |LibraryResource {
                         resource: Resource { hash, size, url },
                         path,
                     }| Source {
                        kind: SourceKind::Library,
                        url: Cow::Borrowed(url),
                        name: path
                            .as_ref()
                            .map(String::as_str)
                            .map(Cow::Borrowed)
                            .unwrap_or_else(|| {
                                Cow::Owned(util::build_library_path(&lib.name, hash, None))
                            }),
                        hash: Some(hash),
                        size: Some(*size),
                    },
                );

                // TODO : filter by OS & arch
                let natives = lib.resources.extra.values().map(
                    |LibraryResource {
                         resource: Resource { hash, size, url },
                         path,
                     }| Source {
                        kind: SourceKind::NativeLibrary,
                        url: Cow::Borrowed(url),
                        name: path
                            .as_ref()
                            .map(String::as_str)
                            .map(Cow::Borrowed)
                            .unwrap_or_else(|| {
                                // TODO
                                let native_str = None;
                                Cow::Owned(util::build_library_path(&lib.name, hash, native_str))
                            }),
                        hash: Some(hash),
                        size: Some(*size),
                    },
                );

                natives.chain(library)
            });

        client_jar
            .chain(server_jar)
            .chain(asset_index)
            .chain(libraries)
    }
}

pub struct JvmList<'manifest> {
    manifest: &'manifest JvmManifest,
}

impl<'manifest> JvmList<'manifest> {
    pub fn new(manifest: &'manifest JvmManifest) -> Self {
        Self { manifest }
    }
}

impl<'manifest> SourceList<'manifest> for JvmList<'manifest> {
    fn sources(self) -> impl Iterator<Item = Source<'manifest>> + 'manifest {
        self.manifest
            .platforms
            .iter()
            .flat_map(|(platform, JvmPlatform { resources })| {
                resources.iter().flat_map(|(jvm_mojang_name, res)| {
                    res.iter().map(
                        |JvmResource {
                             resource: Resource { hash, size, url },
                             version,
                             ..
                         }| {
                            Source {
                                kind: SourceKind::JvmInfo {
                                    platform,
                                    jvm_mojang_name,
                                },
                                url: Cow::Borrowed(url),
                                name: Cow::Borrowed(&version.name),
                                hash: Some(hash),
                                size: Some(*size),
                            }
                        },
                    )
                })
            })
    }
}

pub struct JvmFilesList<'info, 'params> {
    data: &'info JvmInfo,
    jvm_mojang_name: &'params str,
    platform: &'params str,
    prefer_compressed: bool,
}

impl<'info, 'params> JvmFilesList<'info, 'params> {
    pub fn new(
        data: &'info JvmInfo,
        jvm_mojang_name: &'params str,
        platform: &'params str,
        prefer_compressed: bool,
    ) -> Self {
        Self {
            data,
            jvm_mojang_name,
            platform,
            prefer_compressed,
        }
    }
}

impl<'info, 'params: 'info> SourceList<'info> for JvmFilesList<'info, 'params> {
    fn sources(self) -> impl Iterator<Item = Source<'info>> + 'info {
        let Self {
            data,
            jvm_mojang_name,
            platform,
            prefer_compressed,
        } = self;

        data.content
            .iter()
            .filter_map(move |(path, file)| match file {
                JvmContent::File(file) => {
                    let (Resource { hash, size, url }, compressed) = file
                        .downloads
                        .lzma
                        .as_ref()
                        .filter(|_| prefer_compressed)
                        .map(|res| (res, true))
                        .unwrap_or((&file.downloads.raw, false));

                    Some(Source {
                        kind: SourceKind::JvmFile {
                            jvm_mojang_name,
                            platform,
                            executable: file.executable,
                            compressed,
                        },
                        url: Cow::Borrowed(url),
                        name: Cow::Borrowed(path),
                        hash: Some(hash),
                        size: Some(*size),
                    })
                }
                _ => None,
            })
    }
}

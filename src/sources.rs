use std::{borrow::Cow, collections::BTreeMap, iter};

use url::Url;

use crate::{
    metadata::{AssetIndex, AssetMetadata, Sha1Hash, Version, VersionInfo},
    util,
};

#[derive(Debug)]
pub struct Source<'list> {
    pub url: Cow<'list, Url>,
    pub name: Cow<'list, str>,
    pub kind: SourceKind,
    pub hash: Option<&'list Sha1Hash>,
    pub size: Option<u64>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum SourceKind {
    AssetIndex,
    LegacyAsset,
    Asset,
    Library,
    NativeLibrary,
    ClientJar,
    VersionInfo,
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
        let AssetList { origin, index } = self;
        index
            .objects
            .iter()
            .map(move |(path, AssetMetadata { hash, size })| {
                let hash_path = {
                    let hash = hash.to_string();
                    format!("{}/{}", &hash[..2], &hash)
                };
                Source {
                    url: Cow::Owned(origin.join(&hash_path).expect("invalid url-encoded hash")),
                    kind: if index.map_to_resources {
                        SourceKind::LegacyAsset
                    } else {
                        SourceKind::Asset
                    },
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
        let ArtifactList { info, params } = self;
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
        let libraries = info
            .libraries
            .iter()
            .filter(|lib| lib.rules.is_allowed(params))
            .flat_map(|lib| {
                let library = lib.resources.artifact.as_ref().map(|artifact| Source {
                    kind: SourceKind::Library,
                    url: Cow::Borrowed(&artifact.resource.url),
                    name: if let Some(path) = &artifact.path {
                        Cow::Borrowed(path)
                    } else {
                        Cow::Owned(
                            util::build_library_path(&lib.name, None)
                                .unwrap_or_else(|| artifact.resource.hash.to_string()),
                        )
                    },
                    hash: Some(&artifact.resource.hash),
                    size: Some(artifact.resource.size),
                });

                // TODO : filter by OS & arch
                let natives = lib.resources.extra.values().map(|artifact| Source {
                    kind: SourceKind::NativeLibrary,
                    url: Cow::Borrowed(&artifact.resource.url),
                    name: if let Some(path) = &artifact.path {
                        Cow::Borrowed(path)
                    } else {
                        // TODO
                        let native_str = None;
                        Cow::Owned(
                            util::build_library_path(&lib.name, native_str)
                                .unwrap_or_else(|| artifact.resource.hash.to_string()),
                        )
                    },
                    hash: Some(&artifact.resource.hash),
                    size: Some(artifact.resource.size),
                });

                library.into_iter().chain(natives)
            });
        asset_index.chain(client_jar).chain(libraries)
    }
}

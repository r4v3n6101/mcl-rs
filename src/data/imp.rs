use std::iter;
use std::sync::Arc;

use crate::util;

use super::{
    config::{AssetIndexConfig, JvmInfoConfig, VersionInfoConfig},
    mojang::{
        AssetIndex, AssetMetadata, JvmContent, JvmInfo, JvmManifest, JvmPlatform, JvmResource,
        LibraryResource, Resource, VersionInfo, VersionManifest,
    },
    other::{JustFile, ZippedFile},
    Artifact, Source, SourceKind,
};

impl Artifact for JustFile {
    type Config<'this> = ();

    fn provides<'this>(
        &'this self,
        (): Self::Config<'this>,
    ) -> impl Iterator<Item = Source> + 'this {
        iter::empty()
    }
}

impl Artifact for ZippedFile {
    type Config<'this> = ();

    fn provides<'this>(
        &'this self,
        (): Self::Config<'this>,
    ) -> impl Iterator<Item = Source> + 'this {
        // TODO : exclude somehow
        (0..self.archive.len()).map(|i| Source::Archive {
            zipped: self.clone(),
            index: i,
        })
    }
}

impl Artifact for VersionManifest {
    // TODO : selector for versions
    type Config<'this> = ();

    fn provides<'this>(
        &'this self,
        (): Self::Config<'this>,
    ) -> impl Iterator<Item = Source> + 'this {
        self.versions.iter().map(|version| Source::Remote {
            url: Arc::clone(&version.url),
            name: Arc::clone(&version.id),
            kind: SourceKind::VersionInfo,
            hash: None,
            size: None,
        })
    }
}

impl Artifact for AssetIndex {
    type Config<'this> = AssetIndexConfig<'this>;

    fn provides<'this>(
        &'this self,
        config: Self::Config<'this>,
    ) -> impl Iterator<Item = Source> + 'this {
        self.objects
            .iter()
            .map(move |(path, AssetMetadata { hash, size })| {
                let hash_path = {
                    let hash = hash.to_string();
                    format!("{}/{}", &hash[..2], &hash)
                };
                Source::Remote {
                    kind: SourceKind::Asset {
                        legacy: self.map_to_resources,
                    },
                    url: Arc::new(
                        config
                            .origin
                            .join(&hash_path)
                            .expect("couldn't create url with hash"),
                    ),
                    name: if self.map_to_resources {
                        Arc::clone(path)
                    } else {
                        Arc::from(hash_path)
                    },
                    hash: Some(*hash),
                    size: Some(*size),
                }
            })
    }
}

impl Artifact for VersionInfo {
    type Config<'this> = VersionInfoConfig<'this>;

    fn provides<'this>(
        &'this self,
        config: Self::Config<'this>,
    ) -> impl Iterator<Item = Source> + 'this {
        let asset_index = iter::once(Source::Remote {
            kind: SourceKind::AssetIndex,
            url: Arc::clone(&self.asset_index.resource.url),
            name: Arc::clone(&self.asset_index.id),
            hash: Some(self.asset_index.resource.hash),
            size: Some(self.asset_index.resource.size),
        });

        let client_jar = iter::once(Source::Remote {
            kind: SourceKind::ClientJar,
            url: Arc::clone(&self.downloads.client.url),
            name: Arc::clone(&self.id),
            hash: Some(self.downloads.client.hash),
            size: Some(self.downloads.client.size),
        });

        let server_jar = self
            .downloads
            .server
            .as_ref()
            .map(|Resource { hash, size, url }| Source::Remote {
                kind: SourceKind::ServerJar,
                url: Arc::clone(url),
                name: Arc::clone(&self.id),
                hash: Some(*hash),
                size: Some(*size),
            });

        let libraries = self
            .libraries
            .iter()
            .filter(|lib| lib.rules.is_allowed(config.params))
            .flat_map(|lib| {
                let library = lib.resources.artifact.as_ref().map(
                    |LibraryResource {
                         resource: Resource { hash, size, url },
                         path,
                     }| {
                        Source::Remote {
                            kind: SourceKind::Library { zipped: false },
                            url: Arc::clone(url),
                            name: path.as_ref().map_or_else(
                                || Arc::from(util::build_library_path(&lib.name, hash, None)),
                                Arc::clone,
                            ),
                            hash: Some(*hash),
                            size: Some(*size),
                        }
                    },
                );

                // TODO : filter by OS & arch
                let natives = lib.resources.extra.values().map(
                    |LibraryResource {
                         resource: Resource { hash, size, url },
                         path,
                     }| {
                        Source::Remote {
                            kind: SourceKind::Library { zipped: true },
                            url: Arc::clone(url),
                            name: path.as_ref().map_or_else(
                                || {
                                    // TODO
                                    let native_str = None;
                                    Arc::from(util::build_library_path(&lib.name, hash, native_str))
                                },
                                Arc::clone,
                            ),
                            hash: Some(*hash),
                            size: Some(*size),
                        }
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

impl Artifact for JvmManifest {
    type Config<'this> = ();

    fn provides<'this>(
        &'this self,
        (): Self::Config<'this>,
    ) -> impl Iterator<Item = Source> + 'this {
        self.platforms
            .iter()
            .flat_map(|(platform, JvmPlatform { resources })| {
                resources.iter().flat_map(|(jvm_mojang_name, res)| {
                    res.iter().map(
                        |JvmResource {
                             resource: Resource { hash, size, url },
                             version,
                             ..
                         }| {
                            Source::Remote {
                                kind: SourceKind::JvmInfo {
                                    platform: Arc::clone(platform),
                                    jvm_mojang_name: Arc::clone(jvm_mojang_name),
                                },
                                url: Arc::clone(url),
                                name: Arc::clone(&version.name),
                                hash: Some(*hash),
                                size: Some(*size),
                            }
                        },
                    )
                })
            })
    }
}

impl Artifact for JvmInfo {
    type Config<'this> = JvmInfoConfig;

    fn provides<'this>(
        &'this self,
        config: Self::Config<'this>,
    ) -> impl Iterator<Item = Source> + 'this {
        self.content
            .iter()
            .filter_map(move |(path, file)| match file {
                JvmContent::File(file) => {
                    let (Resource { hash, size, url }, compressed) = file
                        .downloads
                        .lzma
                        .as_ref()
                        .filter(|_| config.prefer_compressed)
                        .map_or((&file.downloads.raw, false), |res| (res, true));

                    Some(Source::Remote {
                        kind: SourceKind::JvmFile {
                            jvm_mojang_name: Arc::clone(&config.jvm_mojang_name),
                            platform: Arc::clone(&config.platform),
                            executable: file.executable,
                            compressed,
                        },
                        url: Arc::clone(url),
                        name: Arc::clone(path),
                        hash: Some(*hash),
                        size: Some(*size),
                    })
                }
                _ => None,
            })
    }
}

use std::{array, borrow::Cow, io, iter, sync::Arc};

use bytes::Bytes;
use serde::Serialize;
use yoke::Yoke;

use crate::util;

use super::{
    ArchiveKind, Artifact, GetBytes, Source, SourceKind,
    config::{AssetIndexConfig, JvmInfoConfig, VersionInfoConfig},
    mojang::{
        AssetIndex, AssetMetadata, JvmContent, JvmInfo, JvmManifest, JvmPlatform, JvmResource,
        Library, LibraryResource, OsSelector, Resource, VersionInfo, VersionManifest,
    },
    other::{JustFile, ZipEntry, ZippedNatives},
};

impl<T> GetBytes for T
where
    T: Serialize,
{
    fn calc_bytes(&self) -> io::Result<Bytes> {
        Ok(Bytes::from(serde_json::to_vec_pretty(self)?))
    }
}

impl GetBytes for JustFile {
    fn calc_bytes(&self) -> io::Result<Bytes> {
        Ok(self.data.clone())
    }
}

impl GetBytes for ZippedNatives {
    fn calc_bytes(&self) -> io::Result<Bytes> {
        Ok(self.archive.get_data())
    }
}

impl Artifact for JustFile {
    type Config<'this> = ();

    fn provides<'this>(
        &'this self,
        (): Self::Config<'this>,
    ) -> impl Iterator<Item = Source> + 'this {
        iter::empty()
    }
}

impl Artifact for ZippedNatives {
    type Config<'this> = ();

    fn provides<'this>(
        &'this self,
        (): Self::Config<'this>,
    ) -> impl Iterator<Item = Source> + 'this {
        (0..self.archive.len()).filter_map(|i| {
            // NB: I want try_ returns T: Try
            let entry =
                Yoke::<ZipEntry<'static>, _>::try_attach_to_cart(self.archive.clone(), |archive| {
                    Ok::<_, ()>(ZipEntry {
                        name: archive.name_for_index(i).ok_or(())?,
                    })
                })
                .ok()?;

            if self
                .exclude
                .iter()
                .any(|exclude| entry.get().name.starts_with(&**exclude))
            {
                return None;
            }

            Some(Source::Archive {
                entry,
                kind: ArchiveKind::Natives {
                    classifier: Arc::clone(&self.classifier),
                },
            })
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
            .filter(move |lib| lib.rules.is_allowed(config.params, config.os_selector))
            .flat_map(move |lib| {
                let library = lib.resources.artifact.as_ref().map(
                    |LibraryResource {
                         resource: Resource { hash, size, url },
                         path,
                     }| {
                        Source::Remote {
                            kind: SourceKind::Library,
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

                // TODO : move out?
                let mut natives: [Option<Source>; 6] = array::from_fn(|_| None);
                let variants = [
                    (OsSelector::Linux64, "linux", "64"),
                    (OsSelector::Linux32, "linux", "32"),
                    (OsSelector::OSX64 | OsSelector::MacOS64, "osx", "64"),
                    (OsSelector::OSX32 | OsSelector::MacOS32, "osx", "32"),
                    (
                        OsSelector::Windows64 | OsSelector::Windows10_64,
                        "windows",
                        "64",
                    ),
                    (
                        OsSelector::Windows32 | OsSelector::Windows10_32,
                        "windows",
                        "32",
                    ),
                ];
                for (i, (flag, os_name, arch)) in variants.iter().enumerate() {
                    if config.os_selector.intersects(*flag) {
                        natives[i] = calc_native_str(lib, os_name, arch).map(
                            |(
                                classifier,
                                LibraryResource {
                                    resource: Resource { hash, size, url },
                                    path,
                                },
                            )| {
                                Source::Remote {
                                    kind: SourceKind::ZippedNatives {
                                        classifier: Arc::clone(&self.id),
                                        exclude: Arc::clone(&lib.extract.exclude),
                                    },
                                    url: Arc::clone(url),
                                    name: path.as_ref().map_or_else(
                                        || {
                                            Arc::from(util::build_library_path(
                                                &lib.name,
                                                hash,
                                                Some(&classifier),
                                            ))
                                        },
                                        Arc::clone,
                                    ),
                                    hash: Some(*hash),
                                    size: Some(*size),
                                }
                            },
                        );
                    }
                }

                natives.into_iter().flatten().chain(library)
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

fn calc_native_str<'a>(
    lib: &'a Library,
    os_name: &str,
    bitness: &str,
) -> Option<(Cow<'a, str>, &'a LibraryResource)> {
    lib.natives.get(os_name).and_then(|classifier| {
        let params = iter::once(("arch", bitness)).collect();
        let full_classifier = util::substitute_params(classifier, &params);
        lib.resources
            .extra
            .get(full_classifier.as_ref())
            .map(|res| (full_classifier, res))
    })
}

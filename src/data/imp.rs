use std::{array, borrow::Cow, collections::HashMap, iter, sync::Arc};

use better_any::tid;
use yoke::{Yoke, erased::ErasedArcCart};

use crate::util;

use super::{
    ArchiveKind, Artifact, Source, SourceKind,
    config::{AssetIndexConfig, JvmInfoConfig, VersionInfoConfig},
    mojang::{
        AssetIndex, AssetMetadata, JvmContent, JvmInfo, JvmManifest, JvmPlatform, JvmResource,
        LibraryResource, OsSelector, Resource, VersionInfo, VersionManifest,
    },
    other::{JustFile, ZipEntry, ZippedNatives},
};

// For dyn Tid<'a> casts (i.e. dyn Any with LT bounded)
tid!(VersionManifest<'a>);
tid!(VersionInfo<'a>);
tid!(AssetIndex<'a>);
tid!(JustFile);
tid!(ZippedNatives);

impl Artifact for JustFile {
    type Config = ();

    fn provides(
        &self,
        _: Yoke<Self::Config, ErasedArcCart>,
    ) -> impl Iterator<Item = Source<'_>> + '_ {
        iter::empty()
    }
}

impl Artifact for ZippedNatives {
    type Config = ();

    fn provides(
        &self,
        _: Yoke<Self::Config, ErasedArcCart>,
    ) -> impl Iterator<Item = Source<'_>> + '_ {
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

impl Artifact for VersionManifest<'_> {
    type Config = ();

    fn provides(
        &self,
        _: Yoke<Self::Config, ErasedArcCart>,
    ) -> impl Iterator<Item = Source<'_>> + '_ {
        self.versions.iter().map(|version| Source::Remote {
            url: Arc::clone(&version.url),
            name: Cow::Borrowed(version.id),
            kind: SourceKind::VersionInfo,
            hash: None,
            size: None,
        })
    }
}

impl Artifact for AssetIndex<'_> {
    type Config = AssetIndexConfig<'static>;

    fn provides(
        &self,
        config: Yoke<Self::Config, ErasedArcCart>,
    ) -> impl Iterator<Item = Source<'_>> + '_ {
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
                            .get()
                            .origin
                            .join(&hash_path)
                            .expect("couldn't create url with hash"),
                    ),
                    name: if self.map_to_resources {
                        Cow::Borrowed(path)
                    } else {
                        Cow::Owned(hash_path)
                    },
                    hash: Some(*hash),
                    size: Some(*size),
                }
            })
    }
}

impl Artifact for VersionInfo<'_> {
    type Config = VersionInfoConfig<'static>;

    fn provides(
        &self,
        config: Yoke<Self::Config, ErasedArcCart>,
    ) -> impl Iterator<Item = Source<'_>> + '_ {
        let asset_index = iter::once(Source::Remote {
            kind: SourceKind::AssetIndex,
            url: Arc::clone(&self.asset_index.resource.url),
            name: Cow::Borrowed(self.asset_index.id),
            hash: Some(self.asset_index.resource.hash),
            size: Some(self.asset_index.resource.size),
        });

        let client_jar = iter::once(Source::Remote {
            kind: SourceKind::ClientJar,
            url: Arc::clone(&self.downloads.client.url),
            name: Cow::Borrowed(self.id),
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
                name: Cow::Borrowed(self.id),
                hash: Some(*hash),
                size: Some(*size),
            });

        let os_selector = config.get().os_selector;
        let params: Yoke<&'static HashMap<_, _>, _> = config.map_project(|cfg, _| cfg.params);
        let libraries = self
            .libraries
            .iter()
            .filter(move |lib| lib.rules.is_allowed(params.get(), os_selector))
            .flat_map(move |lib| {
                let library = lib.resources.artifact.as_ref().map(
                    |LibraryResource {
                         resource: Resource { hash, size, url },
                         path,
                     }| {
                        Source::Remote {
                            kind: SourceKind::Library,
                            url: Arc::clone(url),
                            name: path.map_or_else(
                                || Cow::Owned(util::build_library_path(lib.name, &hash, None)),
                                Cow::Borrowed,
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
                    if os_selector.intersects(*flag) {
                        natives[i] = lib.natives.get(os_name).and_then(|classifier| {
                            let params = iter::once(("arch", *arch)).collect();
                            let full_classifier = util::substitute_params(classifier, &params);
                            lib.resources.extra.get(&*full_classifier).map(
                                |LibraryResource {
                                     resource: Resource { hash, size, url },
                                     path,
                                 }| {
                                    Source::Remote {
                                        kind: SourceKind::ZippedNatives {
                                            classifier: Arc::from(self.id),
                                            exclude: &lib.extract.exclude,
                                        },
                                        url: Arc::clone(url),
                                        name: path.map_or_else(
                                            || {
                                                Cow::Owned(util::build_library_path(
                                                    lib.name,
                                                    hash,
                                                    Some(classifier),
                                                ))
                                            },
                                            Cow::Borrowed,
                                        ),
                                        hash: Some(*hash),
                                        size: Some(*size),
                                    }
                                },
                            )
                        });
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

impl Artifact for JvmManifest<'_> {
    type Config = ();

    fn provides(
        &self,
        _: Yoke<Self::Config, ErasedArcCart>,
    ) -> impl Iterator<Item = Source<'_>> + '_ {
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
                                    platform,
                                    jvm_mojang_name,
                                },
                                url: Arc::clone(url),
                                name: Cow::Borrowed(version.name),
                                hash: Some(*hash),
                                size: Some(*size),
                            }
                        },
                    )
                })
            })
    }
}

impl Artifact for JvmInfo<'_> {
    type Config = JvmInfoConfig<'static>;

    fn provides(
        &self,
        config: Yoke<Self::Config, ErasedArcCart>,
    ) -> impl Iterator<Item = Source<'_>> + '_ {
        let prefer_compressed = config.get().prefer_compressed;
        let jvm_mojang_name: Yoke<&'static str, _> =
            config.map_project_cloned(|cfg, _| cfg.jvm_mojang_name);
        let platform: Yoke<&'static str, _> = config.map_project(|cfg, _| cfg.platform);
        self.content
            .iter()
            .filter_map(move |(path, file)| match file {
                JvmContent::File(file) => {
                    let (Resource { hash, size, url }, compressed) = file
                        .downloads
                        .lzma
                        .as_ref()
                        .filter(|_| prefer_compressed)
                        .map_or((&file.downloads.raw, false), |res| (res, true));

                    Some(Source::Remote {
                        kind: SourceKind::JvmFile {
                            jvm_mojang_name: jvm_mojang_name.clone(),
                            platform: platform.clone(),
                            executable: file.executable,
                            compressed,
                        },
                        url: Arc::clone(url),
                        name: Cow::Borrowed(path),
                        hash: Some(*hash),
                        size: Some(*size),
                    })
                }
                _ => None,
            })
    }
}

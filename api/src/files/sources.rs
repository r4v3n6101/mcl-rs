use std::{borrow::Cow, iter};

use reqwest::IntoUrl;

use crate::metadata::{
    assets::{AssetIndex, AssetMetadata},
    game::VersionInfo,
    manifest::Version,
};

use super::{ContentType, Source, SourcesList};

pub fn manifest(url: impl IntoUrl) -> Source<'static> {
    Source {
        r#type: ContentType::VersionManifest,
        url: Cow::Owned(url.into_url().expect("invalid manifest url")),
        name: Cow::Borrowed("manifest"),
        hash: None,
        size: None,
    }
}

impl<'manifest, I> SourcesList<'manifest> for I
where
    I: Iterator<Item = &'manifest Version> + 'manifest,
{
    // impl traits not allowed here for now
    type Iter = Box<dyn Iterator<Item = Source<'manifest>> + 'manifest>;

    fn sources(self) -> Self::Iter {
        Box::new(self.map(|version| Source {
            r#type: ContentType::VersionInfo,
            url: Cow::Borrowed(&version.url),
            name: Cow::Borrowed(&version.id),
            hash: None,
            size: None,
        }))
    }
}

impl<'index> SourcesList<'index> for &'index AssetIndex {
    // impl traits not allowed here for now
    type Iter = Box<dyn Iterator<Item = Source<'index>> + 'index>;

    fn sources(self) -> Self::Iter {
        let is_legacy = self.map_to_resources.unwrap_or(false);
        Box::new(
            self.objects
                .iter()
                .map(move |(path, AssetMetadata { hash, size })| {
                    let hash_path = format!("{}/{}", &hash[..2], &hash);
                    Source {
                        url: Cow::Owned(
                            self.origin
                                .join(&hash_path)
                                .expect("invalid url-encoded hash"),
                        ),
                        r#type: if is_legacy {
                            ContentType::LegacyAsset
                        } else {
                            ContentType::Asset
                        },
                        name: if is_legacy {
                            Cow::Borrowed(path)
                        } else {
                            Cow::Owned(hash_path)
                        },
                        hash: Some(hash),
                        size: Some(*size),
                    }
                }),
        )
    }
}

impl<'info> SourcesList<'info> for &'info VersionInfo {
    // impl traits not allowed here for now
    type Iter = Box<dyn Iterator<Item = Source<'info>> + 'info>;

    fn sources(self) -> Self::Iter {
        let asset_index = iter::once(Source {
            r#type: ContentType::AssetIndex,
            url: Cow::Borrowed(&self.asset_index.resource.url),
            name: Cow::Borrowed(&self.asset_index.id),
            hash: Some(&self.asset_index.resource.sha1),
            size: Some(self.asset_index.resource.size),
        });
        let client_jar = iter::once(Source {
            r#type: ContentType::ClientJar,
            url: Cow::Borrowed(&self.downloads.client.url),
            name: Cow::Borrowed(&self.id),
            hash: Some(&self.downloads.client.sha1),
            size: Some(self.downloads.client.size),
        });
        let libraries = self
            .libraries
            .iter()
            .filter(|lib| lib.is_supported_by_rules())
            .filter_map(|lib| lib.resources.artifact.as_ref())
            .map(|artifact| Source {
                r#type: ContentType::Library,
                url: Cow::Borrowed(&artifact.resource.url),
                name: Cow::Borrowed(&artifact.path),
                hash: Some(&artifact.resource.sha1),
                size: Some(artifact.resource.size),
            });
        let natives = self
            .libraries
            .iter()
            .filter(|lib| lib.is_supported_by_rules())
            .filter_map(|lib| lib.resources.get_native_for_os())
            .map(|artifact| Source {
                r#type: ContentType::NativeLibrary,
                url: Cow::Borrowed(&artifact.resource.url),
                name: Cow::Borrowed(&artifact.path),
                hash: Some(&artifact.resource.sha1),
                size: Some(artifact.resource.size),
            });
        Box::new(
            asset_index
                .chain(client_jar)
                .chain(libraries)
                .chain(natives),
        )
    }
}

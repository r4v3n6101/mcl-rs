use std::{fmt::Debug, iter};

use url::Url;

use crate::metadata::{
    assets::{AssetIndex, AssetMetadata},
    game::VersionInfo,
    manifest::VersionsManifest,
};

use super::Type;

#[derive(Debug)]
pub struct Index<'repo> {
    pub url: Url,
    pub name: &'repo str,
    pub r#type: Type,

    pub hash: Option<&'repo str>,
    pub size: Option<u64>,
}

pub trait Source {
    type IntoIter<'a>: Iterator<Item = Index<'a>>
    where
        Self: 'a;

    fn indices(&self) -> Self::IntoIter<'_>;
}

impl Source for VersionsManifest {
    // impl traits not allowed here for now
    type IntoIter<'a> = Box<dyn Iterator<Item = Index<'a>> + 'a>;

    fn indices(&self) -> Self::IntoIter<'_> {
        Box::new(self.versions.iter().map(|version| Index {
            r#type: Type::VersionInfo,
            url: version.url.clone(),
            name: &version.id,
            hash: None,
            size: None,
        }))
    }
}

impl Source for AssetIndex {
    // impl traits not allowed here for now
    type IntoIter<'a> = Box<dyn Iterator<Item = Index<'a>> + 'a>;

    fn indices(&self) -> Self::IntoIter<'_> {
        Box::new(
            self.objects
                .iter()
                .map(|(path, AssetMetadata { hash, size })| Index {
                    r#type: Type::Asset,
                    url: self
                        .origin
                        .join(&format!("{}/{}", &hash[..2], &hash))
                        .expect("invalid url-encoded hash"),
                    name: &path,
                    hash: Some(&hash),
                    size: Some(*size),
                }),
        )
    }
}

impl Source for VersionInfo {
    // impl traits not allowed here for now
    type IntoIter<'a> = Box<dyn Iterator<Item = Index<'a>> + 'a>;

    fn indices(&self) -> Self::IntoIter<'_> {
        let asset_index = iter::once(Index {
            r#type: Type::AssetIndex,
            url: self.asset_index.resource.url.clone(),
            name: &self.asset_index.id,
            hash: Some(&self.asset_index.resource.sha1),
            size: Some(self.asset_index.resource.size),
        });
        let client_jar = iter::once(Index {
            r#type: Type::ClientJar,
            url: self.downloads.client.url.clone(),
            name: "client.jar",
            hash: Some(&self.downloads.client.sha1),
            size: Some(self.downloads.client.size),
        });
        let libraries = self
            .libraries
            .iter()
            .filter(|lib| lib.is_supported_by_rules())
            .filter_map(|lib| lib.resources.artifact.as_ref())
            .map(|artifact| Index {
                r#type: Type::Library,
                url: artifact.resource.url.clone(),
                name: &artifact.path,
                hash: Some(&artifact.resource.sha1),
                size: Some(artifact.resource.size),
            });
        let natives = self
            .libraries
            .iter()
            .filter(|lib| lib.is_supported_by_rules())
            .filter_map(|lib| lib.resources.get_native_for_os())
            .map(|artifact| Index {
                r#type: Type::NativeLibrary,
                url: artifact.resource.url.clone(),
                name: &artifact.path,
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

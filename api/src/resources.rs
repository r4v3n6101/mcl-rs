use std::io;

use reqwest::IntoUrl;

use crate::metadata::manifest::VersionsManifest;

pub static DEFAULT_RESOURCES_URL: &str = "http://resources.download.minecraft.net";
pub static DEFAULT_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

pub async fn fetch_manifest(url: impl IntoUrl) -> io::Result<VersionsManifest> {
    reqwest::get(url)
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
        .json()
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

use std::borrow::Cow;

use url::Url;

use crate::files::{ContentType, Source};

pub static DEFAULT_RESOURCES_URL: &str = "http://resources.download.minecraft.net";
pub static DEFAULT_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

// Should be static in future
pub fn default_manifest() -> Source<'static> {
    // can't change Owned to Borrowed because Url can't be created constantly
    Source {
        url: Cow::Owned(Url::parse(DEFAULT_MANIFEST_URL).unwrap()),
        name: Cow::Borrowed("default"),
        r#type: ContentType::VersionsManifest,
        hash: None,
        size: None,
    }
}

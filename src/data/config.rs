use std::{collections::HashMap, sync::Arc};

use url::Url;

use super::mojang::OsSelector;

/// Configuration for resolving sub-artifacts from [`AssetIndex`].
pub struct AssetIndexConfig<'cfg> {
    /// Base [`Url`] for downloading of assets.
    pub origin: &'cfg Url,
}

/// Configuration for resolving main data from the [`VersionInfo`].
pub struct VersionInfoConfig<'cfg> {
    /// Features for selecting sub-artifacts.
    /// Primarily helpful in libraries extracting.
    pub params: &'cfg HashMap<&'cfg str, bool>,
    /// Desired OS-es to acquire artifacts for.
    pub os_selector: OsSelector,
}

pub struct JvmInfoConfig {
    pub jvm_mojang_name: Arc<str>,
    pub platform: Arc<str>,
    pub prefer_compressed: bool,
}

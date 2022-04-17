use std::{collections::HashMap, sync::Arc};

use url::Url;

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
    // TODO : native selector and so on
}

pub struct JvmInfoConfig {
    pub jvm_mojang_name: Arc<str>,
    pub platform: Arc<str>,
    pub prefer_compressed: bool,
}

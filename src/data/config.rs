use std::{borrow::Cow, collections::HashMap, sync::Arc};

use url::Url;

use super::mojang::OsSelector;

/// Configuration for resolving sub-artifacts from [`AssetIndex`].
pub struct AssetIndexConfig {
    /// Base [`Url`] for downloading of assets.
    pub origin: Url,
}

/// Configuration for resolving main data from the [`VersionInfo`].
pub struct VersionInfoConfig {
    /// Features for selecting sub-artifacts.
    /// Primarily helpful in libraries extracting.
    pub params: HashMap<Cow<'static, str>, bool>,
    /// Desired OS-es to acquire artifacts for.
    pub os_selector: OsSelector,
}

pub struct JvmInfoConfig {
    pub jvm_mojang_name: Arc<str>,
    pub platform: Arc<str>,
    pub prefer_compressed: bool,
}

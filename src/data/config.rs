use std::{borrow::Cow, collections::HashMap};

use url::Url;
use yoke::Yokeable;

use super::mojang::OsSelector;

/// Configuration for resolving sub-artifacts from [`AssetIndex`].
#[derive(Yokeable)]
pub struct AssetIndexConfig<'cfg> {
    /// Base [`Url`] for downloading of assets.
    pub origin: &'cfg Url,
}

/// Configuration for resolving main data from the [`VersionInfo`].
#[derive(Yokeable)]
pub struct VersionInfoConfig<'cfg> {
    /// Features for selecting sub-artifacts.
    /// Primarily helpful in libraries extracting.
    pub params: &'cfg HashMap<Cow<'static, str>, bool>,
    /// Desired OS-es to acquire artifacts for.
    pub os_selector: OsSelector,
}

#[derive(Yokeable)]
pub struct JvmInfoConfig<'cfg> {
    pub jvm_mojang_name: &'cfg str,
    pub platform: &'cfg str,
    pub prefer_compressed: bool,
}

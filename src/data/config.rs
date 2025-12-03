use std::{collections::HashMap, sync::Arc};

use bitflags::bitflags;
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
    /// Desired OS-es to acquire artifacts for.
    pub os_selector: OsSelector,
}

pub struct JvmInfoConfig {
    pub jvm_mojang_name: Arc<str>,
    pub platform: Arc<str>,
    pub prefer_compressed: bool,
}

bitflags! {
    #[derive(Copy, Clone, PartialEq, Eq)]
    pub struct OsSelector: u32 {
       const Linux64   = 0b0000_0001;
       const Linux32   = 0b0000_0010;
       const OSX64     = 0b0000_0100;
       const OSX32     = 0b0000_1000;
       const Windows64 = 0b0001_0000;
       const Windows32 = 0b0010_0000;
    }
}

use std::{collections::HashMap, str::FromStr};

use serde_derive::Deserialize;

use url::Url;

fn empty_hash() -> String {
    String::from("00null")
}

fn default_base_url() -> Url {
    Url::from_str("http://resources.download.minecraft.net").expect("invalid default resources url")
}

#[derive(Deserialize, Debug, Clone)]
pub struct AssetMetadata {
    #[serde(default = "empty_hash")]
    pub hash: String,
    pub size: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AssetIndex {
    pub map_to_resources: Option<bool>,
    #[serde(default = "default_base_url")]
    pub origin: Url,
    pub objects: HashMap<String, AssetMetadata>,
}

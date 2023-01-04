use chrono::{DateTime, Utc};
use serde_derive::Deserialize;
use url::Url;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseType {
    Release,
    Snapshot,
    OldAlpha,
    OldBeta,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub id: String,
    #[serde(rename = "type")]
    pub release_type: ReleaseType,
    pub url: Url,
    pub time: DateTime<Utc>,
    pub release_time: DateTime<Utc>,
}

#[derive(Deserialize, Debug)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Deserialize, Debug)]
pub struct VersionsManifest {
    pub latest: Latest,
    pub versions: Vec<Version>,
}

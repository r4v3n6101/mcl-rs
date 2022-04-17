use std::{collections::HashMap, iter, sync::Arc};

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_with::{formats::SpaceSeparator, serde_as, OneOrMany, StringWithSeparator};
use url::Url;

pub use sha1_smol::Digest as Sha1Hash;

#[derive(Deserialize, Debug)]
pub struct VersionManifest {
    pub latest: Latest,
    pub versions: Vec<Version>,
}

#[derive(Deserialize, Debug)]
pub struct Latest {
    pub release: Arc<str>,
    pub snapshot: Arc<str>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub id: Arc<str>,
    #[serde(rename = "type")]
    pub version_kind: VersionKind,
    pub url: Arc<Url>,
    pub time: DateTime<Utc>,
    pub release_time: DateTime<Utc>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum VersionKind {
    Release,
    Snapshot,
    OldAlpha,
    OldBeta,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub id: Arc<str>,
    #[serde(rename = "type")]
    pub version_kind: VersionKind,
    pub asset_index: AssetIndexResource,
    pub downloads: Downloads,
    pub libraries: Vec<Library>,
    pub assets: String,
    pub main_class: String,
    #[serde(flatten)]
    pub arguments: Arguments,

    pub minimum_launcher_version: u64,
    pub release_time: DateTime<Utc>,
    pub time: DateTime<Utc>,
    pub java_version: Option<JavaVersion>,
    pub logging: Option<Logging>,
    pub compliance_level: Option<u64>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexResource {
    #[serde(flatten)]
    pub resource: Resource,
    pub id: Arc<str>,
    pub total_size: u64,
}

#[derive(Deserialize, Debug)]
pub struct AssetIndex {
    #[serde(default)]
    pub map_to_resources: bool,
    pub objects: HashMap<Arc<str>, AssetMetadata>,
}

#[derive(Deserialize, Debug)]
pub struct AssetMetadata {
    pub hash: Sha1Hash,
    pub size: u64,
}

#[derive(Deserialize, Debug)]
pub struct Downloads {
    pub client: Resource,
    pub server: Option<Resource>,
}

#[derive(Deserialize, Debug)]
pub struct Library {
    #[serde(rename = "downloads")]
    pub resources: LibraryResources,
    pub name: Arc<str>,
    #[serde(default)]
    pub natives: HashMap<String, String>,
    pub extract: Option<LibraryExtract>,
    #[serde(default)]
    pub rules: Rules,
}

#[derive(Deserialize, Debug)]
pub struct LibraryResources {
    pub artifact: Option<LibraryResource>,
    #[serde(default, rename = "classifiers")]
    pub extra: HashMap<String, LibraryResource>,
}

#[derive(Deserialize, Debug)]
pub struct LibraryResource {
    #[serde(flatten)]
    pub resource: Resource,
    pub path: Option<Arc<str>>,
}

#[derive(Deserialize, Debug)]
pub struct LibraryExtract {
    #[serde(default)]
    pub exclude: Vec<Arc<str>>,
}

#[serde_as]
#[derive(Deserialize, Debug)]
pub enum Arguments {
    #[serde(rename = "arguments")]
    Modern {
        #[serde(default)]
        game: Vec<Argument>,
        #[serde(default)]
        jvm: Vec<Argument>,
    },
    #[serde(rename = "minecraftArguments")]
    Legacy(#[serde_as(as = "StringWithSeparator::<SpaceSeparator, String>")] Vec<String>),
}

#[serde_as]
#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    RuleSpecific {
        #[serde_as(deserialize_as = "OneOrMany<_>")]
        value: Vec<String>,
        #[serde(default)]
        rules: Rules,
    },
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub component: String,
    pub major_version: u32,
}

#[derive(Deserialize, Debug)]
pub struct Logging {
    pub client: LoggerDescription,
}

#[derive(Deserialize, Debug)]
pub struct LoggerDescription {
    pub argument: String,
    #[serde(rename = "type")]
    pub log_type: String,
    #[serde(rename = "file")]
    pub config: LoggerConfig,
}

#[derive(Deserialize, Debug)]
pub struct LoggerConfig {
    #[serde(flatten)]
    pub resource: Resource,
    pub id: String,
}

#[derive(Deserialize, Debug)]
pub struct JvmManifest {
    #[serde(flatten, default)]
    pub platforms: HashMap<Arc<str>, JvmPlatform>,
}

#[derive(Deserialize, Debug)]
pub struct JvmPlatform {
    #[serde(flatten, default)]
    pub resources: HashMap<Arc<str>, Vec<JvmResource>>,
}

#[derive(Deserialize, Debug)]
pub struct JvmResource {
    pub availability: JvmAvailability,
    #[serde(rename = "manifest")]
    pub resource: Resource,
    pub version: JvmVersion,
}

#[derive(Deserialize, Debug)]
pub struct JvmAvailability {
    pub group: u32,
    pub progress: u32,
}

#[derive(Deserialize, Debug)]
pub struct JvmVersion {
    pub name: Arc<str>,
    pub released: DateTime<Utc>,
}

#[derive(Deserialize, Debug)]
pub struct JvmInfo {
    #[serde(rename = "files")]
    pub content: HashMap<Arc<str>, JvmContent>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
pub enum JvmContent {
    File(Box<JvmFile>),
    Link { target: String },
    Directory,
}

#[derive(Deserialize, Debug)]
pub struct JvmFile {
    pub downloads: JvmFileDownloads,
    pub executable: bool,
}

#[derive(Deserialize, Debug)]
pub struct JvmFileDownloads {
    pub lzma: Option<Resource>,
    pub raw: Resource,
}

#[derive(Deserialize, Debug)]
pub struct Resource {
    #[serde(rename = "sha1")]
    pub hash: Sha1Hash,
    pub size: u64,
    pub url: Arc<Url>,
}

#[derive(Deserialize, Debug, Default)]
pub struct Rules(Vec<Rule>);

#[derive(Deserialize, Debug)]
pub struct Rule {
    pub action: RuleAction,
    #[serde(default)]
    pub os: OsDescription,
    #[serde(default)]
    pub features: HashMap<String, bool>,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Disallow,
}

#[derive(Deserialize, Debug, Default)]
pub struct OsDescription {
    pub name: Option<String>,
    pub version: Option<String>,
    pub arch: Option<String>,
}

impl Rules {
    pub fn is_allowed(&self, params: &HashMap<&str, bool>) -> bool {
        !self.0.iter().any(|rule| !rule.is_allowed(params))
    }
}

impl Rule {
    fn calculate_action(&self, params: &HashMap<&str, bool>) -> RuleAction {
        // TODO
        for (k, v) in &self.features {
            if params.get(k.as_str()).unwrap_or(&false) != v {
                return self.action.invert();
            }
        }
        self.action
    }

    pub fn is_allowed(&self, params: &HashMap<&str, bool>) -> bool {
        self.calculate_action(params).value()
    }
}

impl RuleAction {
    pub fn value(self) -> bool {
        match self {
            Self::Allow => true,
            Self::Disallow => false,
        }
    }

    pub fn invert(self) -> Self {
        match self {
            Self::Allow => Self::Disallow,
            Self::Disallow => Self::Allow,
        }
    }
}

impl Arguments {
    pub fn iter_jvm_args<'a>(
        &'a self,
        params: &'a HashMap<&str, bool>,
    ) -> Box<dyn Iterator<Item = &'a str> + 'a> {
        match self {
            Self::Modern { jvm, .. } => Box::new(
                jvm.iter()
                    .flat_map(|argument| argument.iter_strings(params)),
            ),
            Self::Legacy(_) => Box::new(iter::empty()),
        }
    }

    pub fn iter_game_args<'a>(
        &'a self,
        params: &'a HashMap<&str, bool>,
    ) -> Box<dyn Iterator<Item = &'a str> + 'a> {
        match self {
            Self::Modern { game, .. } => Box::new(
                game.iter()
                    .flat_map(|argument| argument.iter_strings(params)),
            ),
            Self::Legacy(s) => Box::new(s.iter().map(String::as_str)),
        }
    }
}

impl Argument {
    pub fn iter_strings<'a>(
        &'a self,
        features: &HashMap<&str, bool>,
    ) -> Box<dyn Iterator<Item = &'a str> + 'a> {
        match self {
            Self::Plain(s) => Box::new(iter::once(s.as_str())),
            Self::RuleSpecific { value, rules } => {
                if rules.is_allowed(features) {
                    Box::new(value.iter().map(String::as_str))
                } else {
                    Box::new(iter::empty())
                }
            }
        }
    }
}

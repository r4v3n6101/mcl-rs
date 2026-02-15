use std::{collections::HashMap, iter, sync::Arc};

use bitflags::bitflags;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{OneOrMany, StringWithSeparator, formats::SpaceSeparator, serde_as};
use url::Url;

pub use sha1_smol::Digest as Sha1Hash;

#[derive(Serialize, Deserialize, Debug)]
pub struct VersionManifest {
    pub latest: Latest,
    pub versions: Vec<Version>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Latest {
    pub release: Arc<str>,
    pub snapshot: Arc<str>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub id: Arc<str>,
    #[serde(rename = "type")]
    pub version_kind: VersionKind,
    pub url: Arc<Url>,
    pub time: DateTime<Utc>,
    pub release_time: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum VersionKind {
    Release,
    Snapshot,
    OldAlpha,
    OldBeta,
}

#[derive(Serialize, Deserialize, Debug)]
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

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexResource {
    #[serde(flatten)]
    pub resource: Resource,
    pub id: Arc<str>,
    pub total_size: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AssetIndex {
    #[serde(default)]
    pub map_to_resources: bool,
    pub objects: HashMap<Arc<str>, AssetMetadata>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AssetMetadata {
    pub hash: Sha1Hash,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Downloads {
    pub client: Resource,
    pub server: Option<Resource>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Library {
    #[serde(rename = "downloads")]
    pub resources: LibraryResources,
    pub name: Arc<str>,
    #[serde(default)]
    pub natives: HashMap<String, String>,
    #[serde(default)]
    pub extract: LibraryExtract,
    #[serde(default)]
    pub rules: Rules,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LibraryResources {
    pub artifact: Option<LibraryResource>,
    #[serde(default, rename = "classifiers")]
    pub extra: HashMap<String, LibraryResource>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LibraryResource {
    #[serde(flatten)]
    pub resource: Resource,
    pub path: Option<Arc<str>>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct LibraryExtract {
    #[serde(default)]
    pub exclude: Arc<[Arc<str>]>,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug)]
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
#[derive(Serialize, Deserialize, Debug)]
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

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub component: String,
    pub major_version: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Logging {
    pub client: LoggerDescription,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LoggerDescription {
    pub argument: String,
    #[serde(rename = "type")]
    pub log_type: String,
    #[serde(rename = "file")]
    pub config: LoggerConfig,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LoggerConfig {
    #[serde(flatten)]
    pub resource: Resource,
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JvmManifest {
    #[serde(flatten, default)]
    pub platforms: HashMap<Arc<str>, JvmPlatform>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JvmPlatform {
    #[serde(flatten, default)]
    pub resources: HashMap<Arc<str>, Vec<JvmResource>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JvmResource {
    pub availability: JvmAvailability,
    #[serde(rename = "manifest")]
    pub resource: Resource,
    pub version: JvmVersion,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JvmAvailability {
    pub group: u32,
    pub progress: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JvmVersion {
    pub name: Arc<str>,
    pub released: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JvmInfo {
    #[serde(rename = "files")]
    pub content: HashMap<Arc<str>, JvmContent>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
pub enum JvmContent {
    File(Box<JvmFile>),
    Link { target: String },
    Directory,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JvmFile {
    pub downloads: JvmFileDownloads,
    pub executable: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JvmFileDownloads {
    pub lzma: Option<Resource>,
    pub raw: Resource,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Resource {
    #[serde(rename = "sha1")]
    pub hash: Sha1Hash,
    pub size: u64,
    pub url: Arc<Url>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Rules(Vec<Rule>);

#[derive(Serialize, Deserialize, Debug)]
pub struct Rule {
    pub action: RuleAction,
    #[serde(default)]
    pub os: OsDescription,
    #[serde(default)]
    pub features: HashMap<String, bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Disallow,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct OsDescription {
    pub name: Option<String>,
    pub version: Option<String>,
    pub arch: Option<String>,
}

bitflags! {
    #[derive(Copy, Clone, PartialEq, Eq)]
    pub struct OsSelector: u32 {
       const Linux32      = 0b0000000001;
       const Linux64      = 0b0000000010;
       const Windows32    = 0b0000000100;
       const Windows64    = 0b0000001000;
       const Windows10_32 = 0b0000010000;
       const Windows10_64 = 0b0000100000;
       const OSX32        = 0b0001000000;
       const OSX64        = 0b0010000000;
       const MacOS32      = 0b0100000000;
       const MacOS64      = 0b1000000000;
    }
}

impl Rules {
    pub fn is_allowed(&self, params: &HashMap<&str, bool>, os_selector: OsSelector) -> bool {
        !self
            .0
            .iter()
            .any(|rule| !rule.is_allowed(params, os_selector))
    }
}

impl Rule {
    fn calculate_action(
        &self,
        params: &HashMap<&str, bool>,
        os_selector: OsSelector,
    ) -> RuleAction {
        let allowed = match (
            self.os.name.as_deref(),
            self.os.arch.as_deref(),
            self.os.version.as_deref(),
        ) {
            (Some("linux"), Some("x86"), _) => OsSelector::Linux32,
            (Some("linux"), _, _) => OsSelector::Linux64 | OsSelector::Linux32,

            (Some("windows"), Some("x86"), Some(version)) if version.starts_with("^10") => {
                OsSelector::Windows10_32
            }
            (Some("windows"), _, Some(version)) if version.starts_with("^10") => {
                OsSelector::Windows10_64 | OsSelector::Windows10_32
            }
            (Some("windows"), Some("x86"), _) => OsSelector::Windows32 | OsSelector::Windows10_32,
            (Some("windows"), _, _) => {
                OsSelector::Windows64
                    | OsSelector::Windows32
                    | OsSelector::Windows10_64
                    | OsSelector::Windows10_32
            }

            (Some("osx"), Some("x86"), Some(version)) if version.starts_with("^10") => {
                OsSelector::OSX32
            }
            (Some("osx"), _, Some(version)) if version.starts_with("^10") => {
                OsSelector::OSX64 | OsSelector::OSX32
            }
            (Some("osx"), Some("x86"), _) => OsSelector::MacOS32 | OsSelector::OSX32,
            (Some("osx"), _, _) => {
                OsSelector::MacOS64 | OsSelector::MacOS32 | OsSelector::OSX64 | OsSelector::OSX32
            }

            _ => OsSelector::all(),
        };

        if !os_selector.intersects(allowed) {
            return self.action.invert();
        }

        for (k, v) in &self.features {
            if params.get(k.as_str()).unwrap_or(&false) != v {
                return self.action.invert();
            }
        }

        self.action
    }

    pub fn is_allowed(&self, params: &HashMap<&str, bool>, os: OsSelector) -> bool {
        self.calculate_action(params, os).value()
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
        os_selector: OsSelector,
    ) -> Box<dyn Iterator<Item = &'a str> + 'a> {
        match self {
            Self::Modern { jvm, .. } => Box::new(
                jvm.iter()
                    .flat_map(move |argument| argument.iter_strings(params, os_selector)),
            ),
            Self::Legacy(_) => Box::new(iter::empty()),
        }
    }

    pub fn iter_game_args<'a>(
        &'a self,
        params: &'a HashMap<&str, bool>,
        os_selector: OsSelector,
    ) -> Box<dyn Iterator<Item = &'a str> + 'a> {
        match self {
            Self::Modern { game, .. } => Box::new(
                game.iter()
                    .flat_map(move |argument| argument.iter_strings(params, os_selector)),
            ),
            Self::Legacy(s) => Box::new(s.iter().map(String::as_str)),
        }
    }
}

impl Argument {
    pub fn iter_strings<'a>(
        &'a self,
        features: &HashMap<&str, bool>,
        os_selector: OsSelector,
    ) -> Box<dyn Iterator<Item = &'a str> + 'a> {
        match self {
            Self::Plain(s) => Box::new(iter::once(s.as_str())),
            Self::RuleSpecific { value, rules } => {
                if rules.is_allowed(features, os_selector) {
                    Box::new(value.iter().map(String::as_str))
                } else {
                    Box::new(iter::empty())
                }
            }
        }
    }
}

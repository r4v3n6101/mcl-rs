use std::{borrow::Cow, collections::HashMap, iter, sync::Arc};

use bitflags::bitflags;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{OneOrMany, StringWithSeparator, formats::SpaceSeparator, serde_as};
use url::Url;

pub use sha1_smol::Digest as Sha1Hash;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionManifest<'a> {
    #[serde(borrow)]
    pub latest: Latest<'a>,
    #[serde(borrow)]
    pub versions: Vec<Version<'a>>,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Latest<'a> {
    #[serde(borrow)]
    pub release: &'a str,
    #[serde(borrow)]
    pub snapshot: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Version<'a> {
    #[serde(borrow)]
    pub id: &'a str,
    #[serde(rename = "type")]
    pub version_kind: VersionKind,
    pub url: Arc<Url>,
    pub time: DateTime<Utc>,
    pub release_time: DateTime<Utc>,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionKind {
    Release,
    Snapshot,
    OldAlpha,
    OldBeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo<'a> {
    #[serde(borrow)]
    pub id: &'a str,
    #[serde(rename = "type")]
    pub version_kind: VersionKind,
    #[serde(borrow)]
    pub asset_index: AssetIndexResource<'a>,
    pub downloads: Downloads,
    #[serde(borrow)]
    pub libraries: Vec<Library<'a>>,
    #[serde(borrow)]
    pub assets: &'a str,
    #[serde(borrow)]
    pub main_class: &'a str,
    #[serde(borrow, flatten)]
    pub arguments: Arguments<'a>,

    pub minimum_launcher_version: u64,
    pub release_time: DateTime<Utc>,
    pub time: DateTime<Utc>,
    #[serde(borrow)]
    pub java_version: Option<JavaVersion<'a>>,
    #[serde(borrow)]
    pub logging: Option<Logging<'a>>,
    pub compliance_level: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexResource<'a> {
    #[serde(flatten)]
    pub resource: Resource,
    #[serde(borrow)]
    pub id: &'a str,
    pub total_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetIndex<'a> {
    #[serde(default)]
    pub map_to_resources: bool,
    #[serde(borrow)]
    pub objects: HashMap<&'a str, AssetMetadata>,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct AssetMetadata {
    pub hash: Sha1Hash,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Downloads {
    pub client: Resource,
    pub server: Option<Resource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library<'a> {
    #[serde(borrow, rename = "downloads")]
    pub resources: LibraryResources<'a>,
    #[serde(borrow)]
    pub name: &'a str,
    #[serde(borrow, default)]
    pub natives: HashMap<&'a str, &'a str>,
    #[serde(borrow, default)]
    pub extract: LibraryExtract<'a>,
    #[serde(borrow, default)]
    pub rules: Rules<'a>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryResources<'a> {
    #[serde(borrow)]
    pub artifact: Option<LibraryResource<'a>>,
    #[serde(borrow, default, rename = "classifiers")]
    pub extra: HashMap<&'a str, LibraryResource<'a>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryResource<'a> {
    #[serde(flatten)]
    pub resource: Resource,
    #[serde(borrow)]
    pub path: Option<&'a str>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LibraryExtract<'a> {
    #[serde(borrow, default)]
    pub exclude: Vec<&'a str>,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Arguments<'a> {
    #[serde(rename = "arguments")]
    Modern {
        #[serde(borrow, default)]
        game: Vec<Argument<'a>>,
        #[serde(borrow, default)]
        jvm: Vec<Argument<'a>>,
    },
    #[serde(rename = "minecraftArguments")]
    Legacy(#[serde_as(as = "StringWithSeparator::<SpaceSeparator, String>")] Vec<String>),
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Argument<'a> {
    Plain(#[serde(borrow)] &'a str),
    RuleSpecific {
        #[serde_as(deserialize_as = "OneOrMany<_>")]
        #[serde(borrow)]
        value: Vec<&'a str>,
        #[serde(borrow, default)]
        rules: Rules<'a>,
    },
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion<'a> {
    #[serde(borrow)]
    pub component: &'a str,
    pub major_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Logging<'a> {
    #[serde(borrow)]
    pub client: LoggerDescription<'a>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggerDescription<'a> {
    #[serde(borrow)]
    pub argument: &'a str,
    #[serde(borrow, rename = "type")]
    pub log_type: &'a str,
    #[serde(borrow, rename = "file")]
    pub config: LoggerConfig<'a>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggerConfig<'a> {
    #[serde(flatten)]
    pub resource: Resource,
    #[serde(borrow)]
    pub id: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JvmManifest<'a> {
    #[serde(borrow, flatten, default)]
    pub platforms: HashMap<&'a str, JvmPlatform<'a>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JvmPlatform<'a> {
    #[serde(borrow, flatten, default)]
    pub resources: HashMap<&'a str, Vec<JvmResource<'a>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JvmResource<'a> {
    pub availability: JvmAvailability,
    #[serde(rename = "manifest")]
    pub resource: Resource,
    #[serde(borrow)]
    pub version: JvmVersion<'a>,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct JvmAvailability {
    pub group: u32,
    pub progress: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JvmVersion<'a> {
    #[serde(borrow)]
    pub name: &'a str,
    pub released: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JvmInfo<'a> {
    #[serde(borrow, rename = "files")]
    pub content: HashMap<&'a str, JvmContent<'a>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
pub enum JvmContent<'a> {
    File(Box<JvmFile>),
    Link {
        #[serde(borrow)]
        target: &'a str,
    },
    Directory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JvmFile {
    pub downloads: JvmFileDownloads,
    pub executable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JvmFileDownloads {
    pub lzma: Option<Resource>,
    pub raw: Resource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    #[serde(rename = "sha1")]
    pub hash: Sha1Hash,
    pub size: u64,
    pub url: Arc<Url>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Rules<'a>(#[serde(borrow)] Vec<Rule<'a>>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule<'a> {
    pub action: RuleAction,
    #[serde(borrow, default)]
    pub os: OsDescription<'a>,
    #[serde(borrow, default)]
    pub features: HashMap<&'a str, bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Disallow,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct OsDescription<'a> {
    #[serde(borrow)]
    pub name: Option<&'a str>,
    #[serde(borrow)]
    pub version: Option<&'a str>,
    #[serde(borrow)]
    pub arch: Option<&'a str>,
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

impl Rules<'_> {
    pub fn is_allowed(
        &self,
        params: &HashMap<Cow<'static, str>, bool>,
        os_selector: OsSelector,
    ) -> bool {
        !self
            .0
            .iter()
            .any(|rule| !rule.is_allowed(params, os_selector))
    }
}

impl Rule<'_> {
    pub fn is_allowed(&self, params: &HashMap<Cow<'static, str>, bool>, os: OsSelector) -> bool {
        self.calculate_action(params, os).value()
    }

    fn calculate_action(
        &self,
        params: &HashMap<Cow<'static, str>, bool>,
        os_selector: OsSelector,
    ) -> RuleAction {
        let allowed = match (self.os.name, self.os.arch, self.os.version) {
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
            if params.get(&Cow::Borrowed(*k)).unwrap_or(&false) != v {
                return self.action.invert();
            }
        }

        self.action
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

impl<'a> Arguments<'a> {
    pub fn iter_jvm_args(
        &'a self,
        params: &'a HashMap<Cow<'static, str>, bool>,
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

    pub fn iter_game_args(
        &'a self,
        params: &'a HashMap<Cow<'static, str>, bool>,
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

impl<'a> Argument<'a> {
    pub fn iter_strings(
        &'a self,
        features: &HashMap<Cow<'static, str>, bool>,
        os_selector: OsSelector,
    ) -> Box<dyn Iterator<Item = &'a str> + 'a> {
        match self {
            Self::Plain(s) => Box::new(iter::once(*s)),
            Self::RuleSpecific { value, rules } => {
                if rules.is_allowed(features, os_selector) {
                    Box::new(value.iter().copied())
                } else {
                    Box::new(iter::empty())
                }
            }
        }
    }
}

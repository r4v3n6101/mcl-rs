use std::{
    collections::HashMap,
    io::{self, Cursor, Read},
    sync::Arc,
};

use bytes::Bytes;
use futures::{stream::FuturesUnordered, StreamExt, TryFutureExt};
use mcl_rs::{
    data::{
        config::{AssetIndexConfig, VersionInfoConfig},
        mojang::{AssetIndex, VersionInfo, VersionManifest},
        other::{JustFile, ZippedFile},
        Source, SourceKind,
    },
    dirs::Dirs,
    resolver::{ErasedArtifact, ResolvedArtifact, ResolvedResult, Resolver},
};
use reqwest::Response;
use serde::de::DeserializeOwned;
use tokio::sync::Semaphore;
use url::Url;
use zip::ZipArchive;

const RESOURCES_URL: &str = "http://resources.download.minecraft.net";
const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

struct SimpleResolver {
    limiter: Arc<Semaphore>,
    dirs: Dirs,
}

struct GlobalConfig {
    resources: Url,
    params: HashMap<&'static str, bool>,
}

impl From<&GlobalConfig> for () {
    fn from(_: &GlobalConfig) -> Self {}
}

impl<'a> From<&'a GlobalConfig> for AssetIndexConfig<'a> {
    fn from(value: &'a GlobalConfig) -> Self {
        Self {
            origin: &value.resources,
        }
    }
}

impl<'a> From<&'a GlobalConfig> for VersionInfoConfig<'a> {
    fn from(value: &'a GlobalConfig) -> Self {
        Self {
            params: &value.params,
        }
    }
}

impl Resolver<GlobalConfig> for SimpleResolver {
    async fn resolve(&self, input: Source) -> ResolvedResult<GlobalConfig> {
        fn decode_json<T: DeserializeOwned>(bytes: &[u8]) -> io::Result<T> {
            serde_json::from_slice(bytes).map_err(Into::into)
        }

        let local_path = self.dirs.locate(&input);
        println!("will be placed at {}", local_path.display());
        let artifact: Arc<dyn ErasedArtifact<GlobalConfig>> = match &input {
            Source::Remote { url, kind, .. } => {
                let _ = self.limiter.acquire().await;

                let data = reqwest::get(url.as_str())
                    .and_then(Response::bytes)
                    .map_err(io::Error::other)
                    .await?;

                match &kind {
                    SourceKind::VersionManifest => Arc::new(decode_json::<VersionManifest>(&data)?),
                    SourceKind::VersionInfo => Arc::new(decode_json::<VersionInfo>(&data)?),
                    SourceKind::AssetIndex => Arc::new(decode_json::<AssetIndex>(&data)?),
                    SourceKind::Library { zipped: true } => Arc::new(ZippedFile {
                        source: Arc::new(input.clone()),
                        archive: ZipArchive::new(Cursor::new(data)).map_err(io::Error::other)?,
                    }),
                    _ => Arc::new(JustFile { data }),
                }
            }
            Source::Archive { zipped, index } => {
                let mut archive = zipped.archive.clone();
                let mut file = archive.by_index(*index).map_err(io::Error::from)?;
                let mut buf = vec![0u8; file.size() as usize];
                file.read_exact(&mut buf)?;

                Arc::new(JustFile {
                    data: Bytes::from(buf),
                })
            }
        };

        Ok(ResolvedArtifact { input, artifact })
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let global_config = GlobalConfig {
        resources: Url::parse(RESOURCES_URL).unwrap(),
        params: Default::default(),
    };
    let resolver = SimpleResolver {
        limiter: Arc::new(Semaphore::new(10)),
        dirs: Dirs {
            root: "/".into(),
            assets: "/assets".into(),
            libraries: "/libraries".into(),
            versions: "/versions".into(),
            runtime: "/runtime".into(),
        },
    };

    let root = Source::Remote {
        url: Arc::new(Url::parse(MANIFEST_URL).unwrap()),
        name: Arc::from("manifest"),
        kind: SourceKind::VersionManifest,
        hash: None,
        size: None,
    };
    let mut tasks = FuturesUnordered::new();
    tasks.push(resolver.resolve(root));

    while let Some(result) = tasks.next().await {
        let Ok(resolved) = result else {
            continue;
        };

        for next in resolved.artifact.provides(&global_config) {
            tasks.push(resolver.resolve(next));
        }
    }
}

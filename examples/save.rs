use std::{
    collections::HashMap,
    io::{self, Cursor, Read},
    sync::Arc,
};

use bytes::BytesMut;
use futures::{StreamExt, TryFutureExt, stream::FuturesUnordered};
use mcl_rs::{
    data::{
        Source, SourceKind,
        config::{AssetIndexConfig, OsSelector, VersionInfoConfig},
        mojang::{AssetIndex, VersionInfo, VersionManifest},
        other::{JustFile, ZippedFile},
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
const VERSION_INFO_URL: &str = "https://piston-meta.mojang.com/v1/packages/ed5d8789ed29872ea2ef1c348302b0c55e3f3468/1.7.10.json";

struct SimpleResolver {
    limiter: Arc<Semaphore>,
    dirs: Dirs,
}

struct GlobalConfig {
    resources: Url,
    params: HashMap<&'static str, bool>,
    os_selector: OsSelector,
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
            os_selector: value.os_selector,
        }
    }
}

impl Resolver<GlobalConfig> for SimpleResolver {
    async fn resolve(&self, input: Source) -> ResolvedResult<GlobalConfig> {
        fn decode_json<T: DeserializeOwned>(bytes: &[u8]) -> io::Result<T> {
            serde_json::from_slice(bytes).map_err(Into::into)
        }

        let _permit = self.limiter.acquire().await;

        let artifact: Arc<dyn ErasedArtifact<GlobalConfig>> = match input {
            Source::Remote {
                ref url, ref kind, ..
            } => {
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
            Source::Archive { ref zipped, index } => {
                let mut archive = zipped.archive.clone();
                let buf = tokio::task::spawn_blocking(move || {
                    let mut file = archive.by_index(index).map_err(io::Error::from).unwrap();
                    let mut buf = BytesMut::zeroed(file.size() as usize);
                    let _ = file.read_exact(&mut buf);

                    buf
                })
                .await
                .unwrap();

                Arc::new(JustFile { data: buf.freeze() })
            }
        };

        Ok(ResolvedArtifact { input, artifact })
    }
}

#[tokio::main]
async fn main() {
    let global_config = GlobalConfig {
        resources: Url::parse(RESOURCES_URL).unwrap(),
        os_selector: OsSelector::all(),
        params: Default::default(),
    };
    let resolver = SimpleResolver {
        limiter: Arc::new(Semaphore::new(10)),
        dirs: Dirs {
            root: "./test_mc/".into(),
            assets: "./test_mc/assets".into(),
            libraries: "./test_mc/libraries".into(),
            versions: "./test_mc/versions".into(),
            runtime: "./test_mc/runtime".into(),
        },
    };

    let root = Source::Remote {
        url: Arc::new(Url::parse(VERSION_INFO_URL).unwrap()),
        name: Arc::from("1.7.10"),
        kind: SourceKind::VersionInfo,
        hash: None,
        size: None,
    };
    save(&resolver, &global_config, root).await;
}

async fn save(resolver: &SimpleResolver, global_config: &GlobalConfig, root: Source) {
    let mut tasks = FuturesUnordered::new();
    tasks.push(resolver.resolve(root));

    while let Some(result) = tasks.next().await {
        let Ok(resolved) = result else {
            continue;
        };

        let local_path = resolver.dirs.locate(&resolved.input);
        let data = resolved.artifact.calc_bytes().unwrap();
        let _ = tokio::fs::create_dir_all(local_path.parent().unwrap()).await;
        let _ = tokio::fs::write(&local_path, &data).await;
        println!("saved: {}", local_path.display());

        for next in resolved.artifact.provides(global_config) {
            tasks.push(resolver.resolve(next));
        }
    }
}

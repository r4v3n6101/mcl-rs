use std::{
    borrow::Cow,
    collections::HashMap,
    io::{self, Read},
    sync::Arc,
};

use bytes::BytesMut;
use futures::{StreamExt, TryFutureExt, stream::FuturesUnordered};
use mcl_rs::{
    data::{
        Source, SourceKind,
        config::{AssetIndexConfig, VersionInfoConfig},
        mojang::{AssetIndex, OsSelector, VersionInfo, VersionManifest},
        other::{JustFile, SharedZipArchive, ZippedNatives},
    },
    dirs::Dirs,
    resolver::{ResolveError, ResolvedArtifact, Resolver},
};
use reqwest::Response;
use tokio::sync::Semaphore;
use url::Url;
use yoke::Yoke;
use zip::ZipArchive;

const RESOURCES_URL: &str = "http://resources.download.minecraft.net";
const VERSION_INFO_URL: &str = "https://piston-meta.mojang.com/v1/packages/ed5d8789ed29872ea2ef1c348302b0c55e3f3468/1.7.10.json";

struct SimpleResolver {
    limiter: Arc<Semaphore>,
    dirs: Dirs,
}

struct GlobalConfig {
    resources: Url,
    params: HashMap<Cow<'static, str>, bool>,
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
    async fn resolve(
        &self,
        input: Source<'_>,
    ) -> Result<Yoke<ResolvedArtifact<'static, GlobalConfig>, Arc<[u8]>>, ResolveError> {
        let _permit = self.limiter.acquire().await;

        match input {
            Source::Remote { url, kind, .. } => {
                let bytes = reqwest::get(url.as_str())
                    .and_then(Response::bytes)
                    .map_err(io::Error::other)
                    .await?;
                let data = Arc::from(bytes.as_ref());

                match &kind {
                    SourceKind::VersionManifest => Yoke::try_attach_to_cart(data, |buf| {
                        serde_json::from_slice::<VersionManifest>(buf)
                            .map(ResolvedArtifact::new)
                            .map_err(move |err| ResolveError::Io(err.into()))
                    }),
                    SourceKind::VersionInfo => Yoke::try_attach_to_cart(data, |buf| {
                        serde_json::from_slice::<VersionInfo>(buf)
                            .map(ResolvedArtifact::new)
                            .map_err(move |err| ResolveError::Io(err.into()))
                    }),
                    SourceKind::AssetIndex => Yoke::try_attach_to_cart(data, |buf| {
                        serde_json::from_slice::<AssetIndex>(buf)
                            .map(ResolvedArtifact::new)
                            .map_err(move |err| ResolveError::Io(err.into()))
                    }),
                    SourceKind::ZippedNatives {
                        classifier,
                        exclude,
                    } => Yoke::try_attach_to_cart(data, |_| {
                        Ok(ResolvedArtifact::new(ZippedNatives {
                            archive: SharedZipArchive::new(bytes).map_err(io::Error::from)?,
                            exclude: exclude.iter().copied().map(str::to_string).collect(),
                            classifier: Arc::clone(classifier),
                        }))
                    }),
                    _ => Ok(Yoke::attach_to_cart(data, |_| {
                        ResolvedArtifact::new(JustFile)
                    })),
                }
            }
            Source::Archive { ref entry, .. } => {
                let entry = entry.clone();
                let buf: BytesMut = tokio::task::spawn_blocking(move || {
                    let name = entry.get().name;
                    let mut archive = ZipArchive::clone(entry.backing_cart());
                    let mut file = archive.by_name(name).map_err(io::Error::other)?;
                    let mut buf = BytesMut::zeroed(file.size() as usize);
                    let _ = file.read_exact(&mut buf);

                    Ok(buf)
                })
                .await
                .map_err(io::Error::other)
                .flatten()?;
                let data = Arc::from(buf.as_ref());

                Ok(Yoke::attach_to_cart(data, |_| {
                    ResolvedArtifact::new(JustFile)
                }))
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let global_config = Arc::new(GlobalConfig {
        resources: Url::parse(RESOURCES_URL).unwrap(),
        os_selector: OsSelector::all(),
        params: Default::default(),
    });
    let resolver = SimpleResolver {
        limiter: Arc::new(Semaphore::new(1000)),
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
        name: Cow::Borrowed("1.7.10"),
        kind: SourceKind::VersionInfo,
        hash: None,
        size: None,
    };
    download(&resolver, global_config, root).await;
}

async fn download(
    resolver: &SimpleResolver,
    global_config: Arc<GlobalConfig>,
    root: Source<'static>,
) {
    let mut stack = vec![save(resolver, root).await];
    while let Some(result) = stack.pop() {
        let Ok(resolved) = result else {
            continue;
        };

        let mut sources = resolved
            .get()
            .artifact
            .provides(global_config.clone())
            .map(|x| save(resolver, x))
            .collect::<FuturesUnordered<_>>();

        while let Some(next) = sources.next().await {
            stack.push(next);
        }
    }
}

async fn save(
    resolver: &SimpleResolver,
    src: Source<'_>,
) -> Result<Yoke<ResolvedArtifact<'static, GlobalConfig>, Arc<[u8]>>, ResolveError> {
    let local_path = resolver.dirs.locate(&src);
    let art_with_data = resolver.resolve(src).await?;
    let bytes = art_with_data.backing_cart();
    let _ = tokio::fs::create_dir_all(local_path.parent().unwrap()).await;
    let _ = tokio::fs::write(&local_path, &bytes).await;
    println!("saved: {}", local_path.display());

    Ok(art_with_data)
}

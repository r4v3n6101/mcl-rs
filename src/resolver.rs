use std::{io, sync::Arc};

use thiserror::Error;

use crate::data::{Artifact, Source};

pub type ResolvedResult<G> = Result<ResolvedArtifact<G>, ResolveError>;

#[derive(Error, Debug)]
pub enum ResolveError {
    // InvalidType {},
    #[error("TODO")]
    Later { input: Source },
    #[error("io error occurred")]
    Io(#[from] io::Error),
}

pub trait Resolver<GlobalConfig> {
    #[allow(async_fn_in_trait)]
    async fn resolve(&self, input: Source) -> ResolvedResult<GlobalConfig>;
}

pub struct ResolvedArtifact<GlobalConfig> {
    pub input: Source,
    pub artifact: Arc<dyn ErasedArtifact<GlobalConfig>>,
}

pub trait ErasedArtifact<GlobalConfig>: Send + Sync + 'static {
    fn provides<'this>(
        &'this self,
        config: &'this GlobalConfig,
    ) -> Box<dyn Iterator<Item = Source> + 'this>;
}

impl<GlobalConfig, T> ErasedArtifact<GlobalConfig> for T
where
    T: Artifact + Send + Sync,
    for<'a> T::Config<'a>: From<&'a GlobalConfig>,
{
    fn provides<'this>(
        &'this self,
        config: &'this GlobalConfig,
    ) -> Box<dyn Iterator<Item = Source> + 'this> {
        Box::new(Artifact::provides(self, config.into()))
    }
}

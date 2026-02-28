use std::{io, mem, sync::Arc};

use better_any::Tid;
use thiserror::Error;
use yoke::{Yoke, Yokeable};

use crate::data::{Artifact, Source};

#[derive(Error, Debug)]
pub enum ResolveError {
    #[error("io error occurred")]
    Io(#[from] io::Error),
}

pub trait Resolver<GlobalConfig: 'static> {
    #[allow(async_fn_in_trait)]
    async fn resolve(
        &self,
        input: Source<'_>,
    ) -> Result<Yoke<ResolvedArtifact<'static, GlobalConfig>, Arc<[u8]>>, ResolveError>;
}

pub struct ResolvedArtifact<'a, G> {
    pub object: Arc<dyn Tid<'a> + 'a>,
    pub artifact: Arc<dyn ErasedArtifact<G> + 'a>,
}

impl<'a, G> ResolvedArtifact<'a, G>
where
    G: Send + Sync + 'static,
{
    pub fn new<A>(artifact: A) -> Self
    where
        A: Artifact + Tid<'a> + 'a,
        for<'b> <<A as Artifact>::Config as Yokeable<'b>>::Output: From<&'b G>,
    {
        let data = Arc::new(artifact);
        Self {
            object: Arc::clone(&data) as Arc<_>,
            artifact: data as Arc<_>,
        }
    }
}

pub trait ErasedArtifact<G> {
    fn provides(&self, config: Arc<G>) -> Box<dyn Iterator<Item = Source<'_>> + '_>;
}

impl<G, T> ErasedArtifact<G> for T
where
    T: Artifact,
    G: Send + Sync + 'static,
    for<'b> <<T as Artifact>::Config as Yokeable<'b>>::Output: From<&'b G>,
{
    fn provides(&self, config: Arc<G>) -> Box<dyn Iterator<Item = Source<'_>> + '_> {
        Box::new(Artifact::provides(
            self,
            Yoke::attach_to_cart(config, |cfg| cfg.into()).erase_arc_cart(),
        ))
    }
}

// SAFETY : it's just fake lifetimes punning
unsafe impl<'a, G: 'static> Yokeable<'a> for ResolvedArtifact<'static, G> {
    type Output = ResolvedArtifact<'a, G>;

    fn transform(&'a self) -> &'a Self::Output {
        unsafe { mem::transmute(self) }
    }

    fn transform_owned(self) -> Self::Output {
        unsafe { mem::transmute(self) }
    }

    unsafe fn make(from: Self::Output) -> Self {
        unsafe { mem::transmute(from) }
    }

    fn transform_mut<F>(&'a mut self, f: F)
    where
        F: 'static + for<'b> FnOnce(&'b mut Self::Output),
    {
        unsafe { f(mem::transmute::<&'a mut Self, &'a mut Self::Output>(self)) }
    }
}

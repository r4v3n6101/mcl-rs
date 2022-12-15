use super::Handle;

pub trait Progress {
    type Output;

    fn progress(&self) -> (Self::Output, Option<Self::Output>);
}

impl<M: Progress, R> Progress for Handle<M, R> {
    type Output = M::Output;

    fn progress(&self) -> (Self::Output, Option<Self::Output>) {
        self.metadata.progress()
    }
}

use std::{
    error::Error,
    fmt::{self, Debug},
    future::Future,
    mem,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::{Arc, RwLock},
};

use tokio::{sync::Semaphore, task::JoinHandle};

pub mod display;
pub mod download;

type StdError = Box<dyn Error + Send + Sync + 'static>;
type Task<R> = Pin<Box<dyn Future<Output = Result<R, StdError>> + Send + Sync + 'static>>;

#[derive(Debug)]
pub struct Cancelled;

impl fmt::Display for Cancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cancelled task")
    }
}

impl Error for Cancelled {}

#[derive(Debug)]
pub enum State<M, R> {
    Pending(fn(Arc<Handle<M, R>>) -> Task<R>),
    Starting,

    Running,
    Paused,

    Finished(R),
    Failed(StdError),

    Cancelled,
}

#[derive(Debug)]
pub struct Handle<M, R> {
    state: RwLock<State<M, R>>,
    metadata: M,
}

impl<M, R> Handle<M, R> {
    pub fn metadata(&self) -> &M {
        &self.metadata
    }

    pub fn state(&self) -> impl Deref<Target = State<M, R>> + '_ {
        self.state.read().unwrap()
    }

    pub fn pause(&self) {
        let mut lock = self.state.write().unwrap();
        let state = lock.deref_mut();
        if let State::Running = state {
            *state = State::Paused;
        }
    }

    pub fn resume(&self) {
        let mut lock = self.state.write().unwrap();
        let state = lock.deref_mut();
        if let State::Paused = state {
            *state = State::Running;
        }
    }

    pub fn cancel(&self) {
        let mut lock = self.state.write().unwrap();
        let state = lock.deref_mut();
        if let State::Running | State::Paused = state {
            *state = State::Cancelled;
        }
    }

    // TODO
    /* pub fn as_result(&self) -> Result<Option<&R>, &StdError> {
        match self.state().deref() {
            State::Finished(res) => Ok(Some(res)),
            State::Failed(err) => Err(err),
            _ => Ok(None),
        }
    } */
}

impl<M, R> Drop for Handle<M, R> {
    fn drop(&mut self) {
        self.cancel()
    }
}

pub struct Manager<M, R = ()> {
    handles: Vec<Arc<Handle<M, R>>>,
    semaphore: Option<Arc<Semaphore>>,
}

impl<M, R> Manager<M, R>
where
    M: Send + Sync + 'static,
    R: Send + Sync + 'static,
{
    pub fn tasks(&self) -> impl Iterator<Item = Arc<Handle<M, R>>> + '_ {
        self.handles.iter().map(|r| Arc::clone(&r))
    }

    pub fn new_task(&mut self, metadata: M, task: fn(Arc<Handle<M, R>>) -> Task<R>) {
        self.handles.push(Arc::new(Handle {
            metadata,
            state: RwLock::new(State::Pending(task)),
        }));
    }

    fn run(&self, handle: &Arc<Handle<M, R>>) -> Option<JoinHandle<()>> {
        let mut lock = handle.state.write().unwrap();
        let state = lock.deref_mut();
        if let State::Pending(_) = state {
            let State::Pending(task) = mem::replace(state, State::Starting) else {
                unreachable!()
            };

            let this = Arc::clone(&handle);
            let semaphore = self.semaphore.as_ref().map(|s| Arc::clone(&s));
            let handle = tokio::spawn(async move {
                let _permit = match semaphore {
                    Some(semaphore) => {
                        Some(semaphore.acquire_owned().await.expect("semaphore closed"))
                    }
                    None => None,
                };
                let res = task(Arc::clone(&this)).await;

                let mut lock = this.state.write().unwrap();
                let state = lock.deref_mut();
                match res {
                    Ok(res) => {
                        *state = State::Finished(res);
                    }
                    Err(err) if err.is::<Cancelled>() => {
                        *state = State::Cancelled;
                    }
                    Err(err) => {
                        *state = State::Failed(err);
                    }
                }
            });
            *state = State::Running;

            Some(handle)
        } else {
            None
        }
    }

    pub async fn run_all(&self) {
        let mut join_handles = Vec::with_capacity(self.handles.len());
        for handle in &self.handles {
            if let Some(join_handle) = self.run(handle) {
                join_handles.push(join_handle);
            }
        }

        for handle in join_handles {
            handle.await.expect("error awaiting tokio task");
        }
    }
}

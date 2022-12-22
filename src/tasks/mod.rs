use std::{
    any::Any,
    error::Error,
    fmt::{self, Display},
    future::Future,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::Arc,
};

use tokio::{
    sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard, Semaphore},
    task::{AbortHandle, JoinSet},
};

pub mod download;

pub type StdError = Box<dyn Error + Send + Sync + 'static>;
pub type Value = Box<dyn Any + Send + Sync>;
pub type Metadata = Box<dyn Any + Send + Sync>;

pub type FutureTask = Pin<Box<dyn Future<Output = Result<Value, StdError>> + Send + Sync>>;

#[derive(Debug, Default)]
pub enum State {
    #[default]
    Pending,
    Running,
    Paused,
    Cancelled,

    Finished(Value),
    Failed(StdError),
}

#[derive(Debug)]
pub struct Cancelled;
impl Display for Cancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cancelled")
    }
}

impl Error for Cancelled {}

#[derive(Debug)]
struct Inner {
    state: State,
    metadata: Metadata,
    abort_handle: Option<AbortHandle>,

    creator: fn(Handle) -> FutureTask,
}

#[derive(Debug, Clone)]
pub struct Handle {
    inner: Arc<RwLock<Inner>>,
}

impl Handle {
    pub fn state(&self) -> impl Deref<Target = State> + '_ {
        RwLockReadGuard::map(self.inner.blocking_read(), |i| &i.state)
    }

    pub fn metadata<T: Any>(&self) -> impl Deref<Target = T> + '_ {
        RwLockReadGuard::map(self.inner.blocking_read(), |i| {
            i.metadata
                .downcast_ref()
                .expect("invalid metadata type provided")
        })
    }

    pub fn metadata_mut<T: Any>(&self) -> impl DerefMut<Target = T> + '_ {
        RwLockWriteGuard::map(self.inner.blocking_write(), |i| {
            i.metadata
                .downcast_mut()
                .expect("invalid metadata type provided")
        })
    }
}

pub struct Manager {
    semaphore: Option<Arc<Semaphore>>,
    handles: RwLock<Vec<Handle>>,
    tasks: Mutex<JoinSet<()>>,
}

impl Manager {
    async fn run_task(
        handle: Handle,
        semaphore: Option<Arc<Semaphore>>,
        generator: fn(Handle) -> FutureTask,
    ) {
        handle.inner.blocking_write().state = State::Pending;
        let _permit = match semaphore {
            Some(semaphore) => Some(semaphore.acquire_owned().await.unwrap()),
            None => None,
        };

        handle.inner.blocking_write().state = State::Running;
        let task = generator(handle.clone());
        let result = task.await;

        match result {
            Ok(val) => handle.inner.blocking_write().state = State::Finished(val),
            Err(err) if err.is::<Cancelled>() => {
                handle.inner.blocking_write().state = State::Cancelled
            }
            Err(err) => handle.inner.blocking_write().state = State::Failed(err),
        }
    }

    fn run(&self, handle: &Handle) {
        let abort_handle = self.tasks.blocking_lock().spawn(Self::run_task(
            handle.clone(),
            self.semaphore.clone(),
            handle.inner.blocking_read().creator,
        ));
        handle.inner.blocking_write().abort_handle = Some(abort_handle);
    }

    pub fn pend_task<T: Any + Send + Sync>(
        &self,
        metadata: T,
        creator: fn(Handle) -> FutureTask,
    ) -> Handle {
        let handle = Handle {
            inner: Arc::new(RwLock::new(Inner {
                creator,
                metadata: Box::new(metadata),
                state: Default::default(),
                abort_handle: None,
            })),
        };
        self.run(&handle);
        self.handles.blocking_write().push(handle.clone());

        handle
    }

    pub fn pause(&self, handle: &Handle) {
        let state = &mut handle.inner.blocking_write().state;
        if matches!(state, State::Running) {
            *state = State::Paused;
        }
    }

    pub fn resume(&self, handle: &Handle) {
        let state = &mut handle.inner.blocking_write().state;
        if matches!(state, State::Paused) {
            *state = State::Running;
        }
    }

    pub fn cancel(&self, handle: &Handle) {
        let inner = &mut handle.inner.blocking_write();
        if matches!(inner.state, State::Paused | State::Running) {
            if let Some(ref abort_handle) = inner.abort_handle {
                abort_handle.abort();
            }
            inner.state = State::Cancelled;
        }
    }

    pub fn restart(&self, handle: &Handle) {
        self.cancel(handle);
        self.run(handle);
    }

    pub async fn wait_all(&self) {
        while self.tasks.lock().await.join_next().await.is_some() {}
    }
}

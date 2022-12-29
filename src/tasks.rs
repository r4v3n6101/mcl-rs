use std::{
    any::Any,
    error::Error,
    fmt::{self, Debug},
    future::{Future, IntoFuture},
    ops::{Deref, DerefMut},
    pin::Pin,
    result,
    sync::{Arc, Mutex, RwLock},
    task::{Context, Poll, Waker},
};

use tokio::{sync::Semaphore, task::JoinSet};
use tracing::{info, info_span, instrument, trace, warn, Instrument};

pub type StdError = Box<dyn Error + Send + Sync + 'static>;
pub type Value = Box<dyn Any + Send + Sync>;
pub type Result = result::Result<Value, StdError>;

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

struct Inner {
    metadata: Box<dyn Any + Send + Sync>,
    state: RwLock<State>,
    waker: Mutex<Option<Waker>>,
}

#[derive(Clone)]
pub struct Handle {
    inner: Arc<Inner>,
}

impl Handle {
    fn state_mut(&self) -> impl DerefMut<Target = State> + '_ {
        self.inner.state.write().unwrap()
    }

    fn waker(&self) -> impl DerefMut<Target = Option<Waker>> + '_ {
        self.inner.waker.lock().unwrap()
    }

    pub fn state(&self) -> impl Deref<Target = State> + '_ {
        self.inner.state.read().unwrap()
    }

    pub fn metadata<T: Any>(&self) -> impl Deref<Target = T> + '_ {
        self.inner
            .metadata
            .downcast_ref()
            .expect("invalid metadata type")
    }

    pub fn pause(&self) {
        let state = &mut *self.state_mut();
        if matches!(state, State::Running) {
            *state = State::Paused;
            if let Some(waker) = self.waker().take() {
                trace!("waking up for pause");
                waker.wake();
            }
        }
    }

    pub fn resume(&self) {
        let state = &mut *self.state_mut();
        if matches!(state, State::Paused) {
            *state = State::Running;
            if let Some(waker) = self.waker().take() {
                trace!("waking up for resume");
                waker.wake();
            }
        }
    }

    pub fn cancel(&self) {
        let state = &mut *self.state_mut();
        if matches!(state, State::Running | State::Paused) {
            *state = State::Cancelled;
            if let Some(waker) = self.waker().take() {
                trace!("waking up for cancel");
                waker.wake();
            }
        }
    }
}

struct Task {
    handle: Handle,
    fut: Pin<Box<dyn Future<Output = Result> + Send + Sync>>,
}

impl Future for Task {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        loop {
            let mut state = this.handle.state_mut();
            match state.deref_mut() {
                State::Pending => {
                    *state = State::Running;
                    continue;
                }
                State::Running => {
                    let fut = Pin::new(&mut this.fut);
                    match fut.poll(cx) {
                        Poll::Ready(res) => {
                            match res {
                                Ok(val) => *state = State::Finished(val),
                                Err(e) => *state = State::Failed(e),
                            }
                            continue;
                        }
                        Poll::Pending => {
                            this.handle.waker().replace(cx.waker().clone());
                        }
                    }
                    return Poll::Pending;
                }
                State::Paused => {
                    this.handle.waker().replace(cx.waker().clone());
                    return Poll::Pending;
                }
                State::Cancelled => {
                    warn!("task cancelled");
                    return Poll::Ready(());
                }
                State::Finished(_) => {
                    info!("successfully finished");
                    return Poll::Ready(());
                }
                State::Failed(_) => {
                    warn!("finished with failure");
                    return Poll::Ready(());
                }
            }
        }
    }
}

#[derive(Default)]
pub struct Manager {
    semaphore: Option<Arc<Semaphore>>,
    tasks: JoinSet<()>,
}

impl Debug for Manager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Manager")
            .field("tasks", &self.tasks())
            .field("permits", &self.permits())
            .finish()
    }
}

impl Manager {
    pub fn with_limit(limit: usize) -> Self {
        Self {
            semaphore: Some(Arc::new(Semaphore::new(limit))),
            ..Default::default()
        }
    }

    pub fn tasks(&self) -> usize {
        self.tasks.len()
    }

    pub fn permits(&self) -> Option<usize> {
        self.semaphore.as_ref().map(|sem| sem.available_permits())
    }

    #[instrument(skip(taskgen))]
    pub fn new_task<T, F>(&mut self, metadata: T, taskgen: fn(Handle) -> F) -> Handle
    where
        T: Any + Debug + Send + Sync,
        F: IntoFuture<Output = Result>,
        <F as IntoFuture>::IntoFuture: Send + Sync + 'static,
    {
        let handle = Handle {
            inner: Arc::new(Inner {
                metadata: Box::new(metadata),
                state: Default::default(),
                waker: Default::default(),
            }),
        };
        let task = Task {
            handle: handle.clone(),
            fut: Box::pin(taskgen(handle.clone()).into_future()),
        };
        let semaphore = self.semaphore.clone();
        self.tasks.spawn(
            async move {
                trace!("trying to acquire permit");
                let _permit = match semaphore {
                    Some(semaphore) => {
                        Some(semaphore.acquire_owned().await.expect("semaphore closed"))
                    }
                    _ => None,
                };
                trace!("permit acquired");
                task.await
            }
            .instrument(info_span!("task_execute")),
        );
        info!("spawned");

        handle
    }

    #[instrument]
    pub async fn wait_all(&mut self) {
        while self.tasks.join_next().await.is_some() {}
    }
}

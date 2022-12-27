use std::{
    any::Any,
    error::Error,
    fmt::Debug,
    future::Future,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::{Arc, Mutex, RwLock},
    task::{Context, Poll, Waker},
};

use tokio::{sync::Semaphore, task::JoinSet};
use tracing::{info, instrument, trace, warn, Instrument};

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
struct Inner {
    metadata: Metadata,
    state: RwLock<State>,
    waker: Mutex<Option<Waker>>,
}

#[derive(Debug, Clone)]
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
                trace!(?self, "waking up for pause");
                waker.wake();
            }
        }
    }

    pub fn resume(&self) {
        let state = &mut *self.state_mut();
        if matches!(state, State::Paused) {
            *state = State::Running;
            if let Some(waker) = self.waker().take() {
                trace!(?self, "waking up for resume");
                waker.wake();
            }
        }
    }

    pub fn cancel(&self) {
        let state = &mut *self.state_mut();
        if matches!(state, State::Running | State::Paused) {
            *state = State::Cancelled;
            if let Some(waker) = self.waker().take() {
                trace!(?self, "waking up for cancel");
                waker.wake();
            }
        }
    }
}

struct Task {
    handle: Handle,
    fut: FutureTask,
}

impl Future for Task {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        loop {
            let mut state = this.handle.state_mut();
            match state.deref_mut() {
                State::Pending => {
                    trace!(?this.handle, "poll pending");
                    *state = State::Running;
                    continue;
                }
                State::Running => {
                    trace!(?this.handle, "poll running");
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
                            this.handle
                                .waker()
                                .replace(cx.waker().clone());
                        }
                    }
                    return Poll::Pending;
                }
                State::Paused => {
                    trace!(?this.handle, "poll paused");
                    this.handle
                        .waker()
                        .replace(cx.waker().clone());
                    return Poll::Pending;
                }
                State::Cancelled => {
                    warn!(?this.handle, "task cancelled");
                    return Poll::Ready(());
                }
                State::Finished(_) => {
                    info!(?this.handle, "successfully finished");
                    return Poll::Ready(());
                }
                State::Failed(_) => {
                    warn!(?this.handle, "finished with failure");
                    return Poll::Ready(());
                }
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct Manager {
    semaphore: Option<Arc<Semaphore>>,
    tasks: JoinSet<()>,
}

impl Manager {
    pub fn with_limit(limit: usize) -> Self {
        Self {
            semaphore: Some(Arc::new(Semaphore::new(limit))),
            ..Default::default()
        }
    }

    #[instrument]
    pub fn new_task<T: Any + Debug + Send + Sync>(
        &mut self,
        metadata: T,
        taskgen: fn(Handle) -> FutureTask,
    ) -> Handle {
        let handle = Handle {
            inner: Arc::new(Inner {
                metadata: Box::new(metadata),
                state: Default::default(),
                waker: Default::default(),
            }),
        };
        let task = Task {
            handle: handle.clone(),
            fut: taskgen(handle.clone()),
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
            .in_current_span(),
        );
        info!(?handle, "new task spawned");

        handle
    }

    #[instrument]
    pub async fn wait_all(&mut self) {
        while self.tasks.join_next().await.is_some() {}
    }
}

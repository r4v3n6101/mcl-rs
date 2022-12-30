use std::{
    fmt::{self, Debug},
    future::{Future, IntoFuture},
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::{Arc, Mutex, RwLock},
    task::{Context, Poll, Waker},
};

use tokio::{sync::Semaphore, task::JoinSet};
use tracing::{info, info_span, instrument, trace, warn, Instrument};

pub enum State<V, E> {
    Pending,
    Running,
    Paused,
    Cancelled,

    Finished(V),
    Failed(E),
}

impl<V, E> Debug for State<V, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match &self {
            State::Pending => "Pending",
            State::Running => "Running",
            State::Paused => "Paused",
            State::Cancelled => "Cancelled",
            State::Finished(_) => "Finished",
            State::Failed(_) => "Failed",
        })
    }
}

struct Inner<M, V, E> {
    metadata: M,
    state: RwLock<State<V, E>>,
    waker: Mutex<Option<Waker>>,
}

pub struct Handle<M, V, E> {
    inner: Arc<Inner<M, V, E>>,
}

impl<M, V, E> Clone for Handle<M, V, E> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<M: Debug, V, E> Debug for Handle<M, V, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Handle")
            .field("metadata", &self.inner.metadata)
            // TODO : may deadlock
            .field("state", self.state().deref())
            .finish()
    }
}

impl<M, V, E> Handle<M, V, E> {
    fn waker(&self) -> impl DerefMut<Target = Option<Waker>> + '_ {
        self.inner.waker.lock().unwrap()
    }

    fn state_mut(&self) -> impl DerefMut<Target = State<V, E>> + '_ {
        self.inner.state.write().unwrap()
    }

    pub fn state(&self) -> impl Deref<Target = State<V, E>> + '_ {
        self.inner.state.read().unwrap()
    }

    pub fn metadata(&self) -> &M {
        &self.inner.metadata
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

struct Task<M, V, E, F> {
    handle: Handle<M, V, E>,
    fut: F,
}

impl<M, V, E, F> Future for Task<M, V, E, F>
where
    M: Debug,
    F: Future<Output = Result<V, E>> + Unpin,
{
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
                            this.handle.waker().replace(cx.waker().clone());
                        }
                    }
                    return Poll::Pending;
                }
                State::Paused => {
                    trace!(?this.handle, "poll paused");

                    this.handle.waker().replace(cx.waker().clone());
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
    pub fn new_task<M, V, E, F>(
        &mut self,
        metadata: M,
        taskgen: fn(Handle<M, V, E>) -> F,
    ) -> Handle<M, V, E>
    where
        M: Debug + Send + Sync + 'static,
        V: Send + Sync + 'static,
        E: Send + Sync + 'static,
        F: IntoFuture<Output = Result<V, E>>,
        <F as IntoFuture>::IntoFuture: Send + 'static,
    {
        let handle = Handle {
            inner: Arc::new(Inner {
                metadata,
                state: RwLock::new(State::Pending),
                waker: Mutex::new(None),
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

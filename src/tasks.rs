use std::{
    fmt::{self, Debug},
    future::{Future, IntoFuture},
    ops::Deref,
    pin::Pin,
    sync::{Arc, Mutex, RwLock},
    task::{Context, Poll, Waker},
};

use crossbeam_utils::atomic::AtomicCell;
use tokio::{sync::Semaphore, task::JoinSet};
use tracing::{info, info_span, instrument, trace, warn, Instrument};

#[derive(Default, Debug, Copy, Clone)]
pub enum State {
    #[default]
    Pending,
    Running,
    Paused,
    Cancelled,

    Finished,
    Failed,
}

struct Inner<M, V, E> {
    metadata: M,
    state: AtomicCell<State>,
    waker: Mutex<Option<Waker>>,
    result: RwLock<Option<Result<V, E>>>,
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
            .field("state", &self.inner.state)
            .finish()
    }
}

impl<M, V, E> Handle<M, V, E> {
    fn change_result(&self, res: Result<V, E>) {
        self.inner.result.write().unwrap().replace(res);
    }

    fn change_state(&self, state: State) {
        self.inner.state.store(state)
    }

    fn change_waker(&self, waker: Waker) {
        self.inner.waker.lock().unwrap().replace(waker);
    }

    fn wakeup(&self) {
        if let Some(waker) = self.inner.waker.lock().unwrap().take() {
            trace!("waking up task");
            waker.wake();
        } else {
            warn!("no waker");
        }
    }

    pub fn state(&self) -> State {
        self.inner.state.load()
    }

    pub fn metadata(&self) -> &M {
        &self.inner.metadata
    }

    pub fn result(&self) -> impl Deref<Target = Option<Result<V, E>>> + '_ {
        self.inner.result.read().unwrap()
    }

    pub fn pause(&self) {
        if matches!(self.state(), State::Running) {
            self.change_state(State::Paused);
            self.wakeup();
        }
    }

    pub fn resume(&self) {
        if matches!(self.state(), State::Paused) {
            self.change_state(State::Running);
            self.wakeup();
        }
    }

    pub fn cancel(&self) {
        if matches!(self.state(), State::Running | State::Paused) {
            self.change_state(State::Cancelled);
            self.wakeup();
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
            match this.handle.state() {
                State::Pending => {
                    trace!(?this.handle, "poll pending");

                    this.handle.change_state(State::Running);
                    continue;
                }
                State::Running => {
                    trace!(?this.handle, "poll running");

                    let fut = Pin::new(&mut this.fut);
                    match fut.poll(cx) {
                        Poll::Ready(res) => {
                            match res {
                                Ok(val) => {
                                    this.handle.change_state(State::Finished);
                                    this.handle.change_result(Ok(val));
                                }
                                Err(e) => {
                                    this.handle.change_state(State::Failed);
                                    this.handle.change_result(Err(e));
                                }
                            }
                            continue;
                        }
                        Poll::Pending => {
                            this.handle.change_waker(cx.waker().clone());
                        }
                    }
                    return Poll::Pending;
                }
                State::Paused => {
                    trace!(?this.handle, "poll paused");

                    this.handle.change_waker(cx.waker().clone());
                    return Poll::Pending;
                }
                State::Cancelled => {
                    warn!(?this.handle, "task cancelled");
                    return Poll::Ready(());
                }
                State::Finished => {
                    info!(?this.handle, "successfully finished");
                    return Poll::Ready(());
                }
                State::Failed => {
                    warn!(?this.handle, "finished with failure");
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
                state: Default::default(),
                waker: Default::default(),
                result: Default::default(),
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

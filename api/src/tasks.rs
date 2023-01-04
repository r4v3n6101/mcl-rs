use std::{
    fmt::{self, Debug},
    future::Future,
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
}

struct Inner<M, R> {
    metadata: M,
    state: AtomicCell<State>,
    waker: Mutex<Option<Waker>>,
    result: RwLock<Option<R>>,
}

pub struct Handle<M, R> {
    inner: Arc<Inner<M, R>>,
}

impl<M, R> Clone for Handle<M, R> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<M, R> Deref for Handle<M, R> {
    type Target = M;

    fn deref(&self) -> &Self::Target {
        &self.inner.metadata
    }
}

impl<M: Debug, R> Debug for Handle<M, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Handle")
            .field("metadata", &self.inner.metadata)
            .field("state", &self.inner.state)
            .finish()
    }
}

impl<M, R> Handle<M, R> {
    fn change_result(&self, result: R) {
        self.inner.result.write().unwrap().replace(result);
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

    pub fn result(&self) -> impl Deref<Target = Option<R>> + '_ {
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

struct Task<M, R, F> {
    handle: Handle<M, R>,
    fut: F,
}

impl<M, R, F> Future for Task<M, R, F>
where
    M: Debug,
    F: Future<Output = R> + Unpin,
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
                            this.handle.change_state(State::Finished);
                            this.handle.change_result(res);
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
                State::Finished | State::Cancelled => {
                    info!(?this.handle, "task finished");
                    return Poll::Ready(());
                }
            }
        }
    }
}

pub trait GenerateTask: Sized {
    type Output;
    type Future: Future<Output = Self::Output> + Send + Unpin;

    fn task(handle: Handle<Self, Self::Output>) -> Self::Future;
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

    #[instrument]
    pub fn new_task<M, R>(&mut self, metadata: M) -> Handle<M, R>
    where
        R: Send + Sync + 'static,
        M: GenerateTask<Output = R> + Debug + Send + Sync + 'static,
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
            fut: M::task(handle.clone()),
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

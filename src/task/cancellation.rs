use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

#[derive(Debug, Clone)]
pub struct CancellationToken {
    inner: Arc<CancellationTokenInner>,
}

#[derive(Debug)]
struct CancellationTokenInner {
    cancelled: AtomicBool,
    parent: Mutex<Option<CancellationToken>>,
    children: Mutex<Vec<CancellationToken>>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(CancellationTokenInner {
                cancelled: AtomicBool::new(false),
                parent: Mutex::new(None),
                children: Mutex::new(Vec::new()),
            }),
        }
    }

    pub fn cancel(&self) {
        self.inner.cancelled.store(true, Ordering::SeqCst);
        let children = self.inner.children.lock().unwrap();
        for child in children.iter() {
            child.cancel();
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::SeqCst)
    }

    pub fn child_token(&self) -> Self {
        let child = Self::new();
        *child.inner.parent.lock().unwrap() = Some(self.clone());
        let mut children = self.inner.children.lock().unwrap();
        children.push(child.clone());
        child
    }

    pub fn cancelled(&self) -> CancelledFuture {
        CancelledFuture {
            token: self.clone(),
        }
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct CancelledFuture {
    token: CancellationToken,
}

impl Future for CancelledFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.token.is_cancelled() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

#[derive(Debug, Clone)]
pub struct TaskGroup {
    token: CancellationToken,
}

impl TaskGroup {
    pub fn new() -> Self {
        Self {
            token: CancellationToken::new(),
        }
    }

    pub fn spawn<F>(
        &self,
        runtime: &crate::Runtime,
        future: F,
    ) -> crate::task::handle::TaskHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let token = self.token.clone();
        let wrapped = CancellableFuture::new(future, token);
        runtime.spawn_task(wrapped).expect("failed to spawn task")
    }

    pub fn cancel(&self) {
        self.token.cancel();
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.token.clone()
    }

    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }
}

impl Default for TaskGroup {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct CancellableFuture<F> {
    future: F,
    token: CancellationToken,
}

impl<F> CancellableFuture<F> {
    pub fn new(future: F, token: CancellationToken) -> Self {
        Self { future, token }
    }
}

impl<F: Future> Future for CancellableFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.token.is_cancelled() {
            panic!("task cancelled");
        }
        let this = unsafe { self.get_unchecked_mut() };
        let future = unsafe { Pin::new_unchecked(&mut this.future) };
        future.poll(cx)
    }
}

//! Shared tokio runtime.
use std::future::Future;
use std::sync::LazyLock;
use tokio::runtime::Runtime;

static RT: LazyLock<Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime")
});

pub fn rt() -> &'static Runtime {
    &RT
}

pub fn spawn<F>(f: F) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    RT.spawn(f)
}

pub fn spawn_blocking<F, R>(f: F) -> tokio::task::JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    RT.spawn_blocking(f)
}

pub fn block_on<F: Future>(f: F) -> F::Output {
    RT.block_on(f)
}

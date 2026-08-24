//! Async task helpers bridging core async operations with the immediate-mode UI.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock};

/// The shared background runtime for network operations.
pub fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("hmcl-worker")
            .enable_all()
            .build()
            .expect("failed to start the background runtime")
    })
}

/// A pollable handle to a background task.
#[derive(Clone)]
pub struct AsyncTask<T: Send> {
    result: Arc<Mutex<Option<Result<T, String>>>>,
}

impl<T: Send> AsyncTask<T> {
    /// Poll the task result. `None` means it is still running.
    pub fn poll(&self) -> Option<Result<T, String>> {
        self.result.lock().unwrap().take()
    }

    pub fn is_done(&self) -> bool {
        self.result.lock().unwrap().is_some()
    }
}

/// Spawn `future` on the shared runtime, returning a pollable handle.
pub fn spawn<T, F>(future: F) -> AsyncTask<T>
where
    T: Send + 'static,
    F: Future<Output = Result<T, String>> + Send + 'static,
{
    let result: Arc<Mutex<Option<Result<T, String>>>> = Arc::new(Mutex::new(None));
    let handle = AsyncTask {
        result: result.clone(),
    };
    runtime().spawn(async move {
        let value = future.await;
        *result.lock().unwrap() = Some(value);
    });
    handle
}

/// Spawn a boxed future with an owned error message.
pub fn spawn_boxed<T: Send + 'static>(
    future: Pin<Box<dyn Future<Output = anyhow::Result<T>> + Send>>,
) -> AsyncTask<T> {
    spawn(async move { future.await.map_err(|e| format!("{e:#}")) })
}

/// Convenience: run a blocking closure on the background runtime.
pub fn spawn_blocking<T, F>(f: F) -> AsyncTask<T>
where
    T: Send + 'static,
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
{
    spawn(async move { f().map_err(|e| format!("{e:#}")) })
}

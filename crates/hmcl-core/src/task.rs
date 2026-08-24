//! Async task framework mirroring HMCL's `org.jackhuang.hmcl.task` package.
//!
//! Tasks are cancellable units of work reporting progress to a listener.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[derive(Debug, Clone, Default)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
    pub message: Option<String>,
}

impl Progress {
    pub fn ratio(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.done as f32 / self.total as f32
        }
    }
}

#[derive(Debug, Default)]
pub struct CancellationToken {
    cancelled: AtomicBool,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn check(&self) -> anyhow::Result<()> {
        if self.is_cancelled() {
            anyhow::bail!("task cancelled");
        }
        Ok(())
    }
}

pub type TaskFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>;

/// A cancellable, progress-reporting unit of work.
pub trait Task: Send {
    fn execute(
        &mut self,
        progress: ProgressTracker,
        cancellation: CancellationToken,
    ) -> TaskFuture<'_>;
}

#[derive(Clone, Default)]
pub struct ProgressTracker(Arc<AtomicU64>);

impl ProgressTracker {
    pub fn add(&self, delta: u64) {
        self.0.fetch_add(delta, Ordering::SeqCst);
    }

    pub fn get(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

//! Async event bus mirroring HMCL's `org.jackhuang.hmcl.event` package.

use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub type EventHandler = Arc<dyn Fn(&dyn Any) + Send + Sync>;

/// A simple synchronous event bus. Handlers are keyed by event type id.
#[derive(Default, Clone)]
pub struct EventBus {
    handlers: Arc<Mutex<HashMap<std::any::TypeId, Vec<EventHandler>>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe<T: 'static>(&self, handler: impl Fn(&T) + Send + Sync + 'static) {
        let wrapped: EventHandler = Arc::new(move |event: &dyn Any| {
            if let Some(event) = event.downcast_ref::<T>() {
                handler(event);
            }
        });
        self.handlers
            .lock()
            .unwrap()
            .entry(std::any::TypeId::of::<T>())
            .or_default()
            .push(wrapped);
    }

    pub fn fire<T: 'static + Send + Sync>(&self, event: T) {
        let handlers = self
            .handlers
            .lock()
            .unwrap()
            .get(&std::any::TypeId::of::<T>())
            .cloned()
            .unwrap_or_default();
        for handler in handlers {
            handler(&event);
        }
    }
}

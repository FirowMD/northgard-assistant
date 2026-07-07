use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

struct EventSlot {
    flag: AtomicBool,
    payload: Mutex<Option<Box<dyn Any + Send + Sync>>>,
}

impl EventSlot {
    fn new() -> Self {
        Self { flag: AtomicBool::new(false), payload: Mutex::new(None) }
    }
}

pub struct EventManager {
    handlers: Mutex<HashMap<TypeId, Vec<Arc<dyn Fn(&dyn Any) + Send + Sync>>>>,
    slots: Mutex<HashMap<TypeId, Arc<EventSlot>>>,
}

impl EventManager {
    pub fn new() -> Self {
        Self {
            handlers: Mutex::new(HashMap::new()),
            slots: Mutex::new(HashMap::new()),
        }
    }

    pub fn register<T: Any + Send + Sync, F>(&self, handler: F)
    where
        F: Fn(&T) + Send + Sync + 'static,
    {
        {
            let mut slots = self.slots.lock().unwrap();
            slots.entry(TypeId::of::<T>()).or_insert_with(|| Arc::new(EventSlot::new()));
        }

        let mut handlers = self.handlers.lock().unwrap();
        let entry = handlers.entry(TypeId::of::<T>()).or_insert_with(Vec::new);
        entry.push(Arc::new(move |event| {
            if let Some(event) = event.downcast_ref::<T>() {
                handler(event);
            }
        }));
    }

    pub fn emit<T: Any + Send + Sync>(&self, event: T) {
        let type_id = TypeId::of::<T>();
        let slot = {
            let mut slots = self.slots.lock().unwrap();
            slots
                .entry(type_id)
                .or_insert_with(|| Arc::new(EventSlot::new()))
                .clone()
        };

        {
            let mut payload = slot.payload.lock().unwrap();
            *payload = Some(Box::new(event));
        }
        slot.flag.store(true, Ordering::SeqCst);
    }

    pub fn update(&self) {
        let slots_snapshot: Vec<(TypeId, Arc<EventSlot>)> = {
            let slots = self.slots.lock().unwrap();
            slots.iter().map(|(k, v)| (*k, v.clone())).collect()
        };

        for (type_id, slot) in slots_snapshot {
            if slot.flag.swap(false, Ordering::SeqCst) {
                let payload_opt = {
                    let mut payload = slot.payload.lock().unwrap();
                    payload.take()
                };

                if let Some(payload) = payload_opt {
                    let handlers_to_call: Vec<Arc<dyn Fn(&dyn Any) + Send + Sync>> = {
                        let handlers = self.handlers.lock().unwrap();
                        handlers.get(&type_id).cloned().unwrap_or_default()
                    };

                    let any_ref: &dyn Any = payload.as_ref();
                    for handler in handlers_to_call {
                        handler(any_ref);
                    }
                }
            }
        }
    }
}

pub fn instance() -> &'static Arc<EventManager> {
    static INSTANCE: once_cell::sync::Lazy<Arc<EventManager>> = once_cell::sync::Lazy::new(|| Arc::new(EventManager::new()));
    &INSTANCE
}
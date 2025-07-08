use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::any::Any;

type SlotId = u32;
type Callback = Box<dyn Fn(&dyn Any) + Send + 'static>;

pub struct Signal {
    slots: Arc<Mutex<HashMap<SlotId, Callback>>>,
    next_id: Arc<Mutex<SlotId>>,
}

impl Signal {
    pub fn new() -> Self {
        Self {
            slots: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(0)),
        }
    }

    pub fn connect<F>(&self, callback: F) -> SlotId 
    where
        F: Fn(&dyn Any) + Send + 'static,
    {
        let mut next_id = self.next_id.lock().unwrap();
        let id = *next_id;
        *next_id += 1;

        self.slots.lock().unwrap().insert(id, Box::new(callback));
        id
    }

    pub fn disconnect(&self, id: SlotId) -> bool {
        self.slots.lock().unwrap().remove(&id).is_some()
    }

    pub fn emit(&self, data: &dyn Any) {
        let slots = self.slots.lock().unwrap();
        for callback in slots.values() {
            callback(data);
        }
    }
}

impl Default for Signal {
    fn default() -> Self {
        Self::new()
    }
}

// Example usage with type-safe wrapper
pub struct TypedSignal<T: 'static> {
    signal: Signal,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: 'static> TypedSignal<T> {
    pub fn new() -> Self {
        Self {
            signal: Signal::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn connect<F>(&self, callback: F) -> SlotId 
    where
        F: Fn(&T) + Send + 'static,
    {
        self.signal.connect(move |data| {
            if let Some(typed_data) = data.downcast_ref::<T>() {
                callback(typed_data);
            }
        })
    }

    pub fn disconnect(&self, id: SlotId) -> bool {
        self.signal.disconnect(id)
    }

    pub fn emit(&self, data: &T) {
        self.signal.emit(data)
    }
}

impl<T: 'static> Default for TypedSignal<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI32, Ordering};

    #[test]
    fn test_typed_signal() {
        let signal = TypedSignal::<i32>::new();
        let counter = Arc::new(AtomicI32::new(0));
        let counter_clone = counter.clone();

        let id = signal.connect(move |&value| {
            counter_clone.fetch_add(value, Ordering::SeqCst);
        });

        signal.emit(&5);
        assert_eq!(counter.load(Ordering::SeqCst), 5);

        signal.disconnect(id);
        signal.emit(&3);
        assert_eq!(counter.load(Ordering::SeqCst), 5);
    }
}

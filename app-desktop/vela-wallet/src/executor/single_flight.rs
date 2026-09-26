//! One read in flight per question.
//!
//! The fee session for the speed in force and a preview session for each of
//! the other speeds (spec 069) ask the SAME reads at the same instant — the
//! deployment status, the gas signals, the relay's gas quote, the in-band
//! rows, the simulation of one exact operation. The caches beside those reads
//! only help the NEXT asker; three askers arriving together each missed them
//! and each went to the network, so the three speed rows landed one after
//! another, as each of three identical requests happened to come back.
//!
//! Here the first asker reads and the rest wait for its answer: one request,
//! every row settled by it at the same moment.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Condvar, Mutex, PoisonError};

enum Slot<V> {
    Waiting,
    Done(V),
    /// The reader panicked; a waiter reads for itself rather than hang.
    Abandoned,
}

type Shared<V> = Arc<(Mutex<Slot<V>>, Condvar)>;

pub struct SingleFlight<K, V> {
    in_flight: Mutex<Option<HashMap<K, Shared<V>>>>,
}

impl<K: Eq + Hash + Clone, V: Clone> SingleFlight<K, V> {
    pub const fn new() -> Self {
        Self {
            in_flight: Mutex::new(None),
        }
    }

    /// `read()` once for everybody asking `key` while it runs.
    pub fn run(&self, key: K, read: impl FnOnce() -> V) -> V {
        let (shared, leader) = {
            let mut guard = self
                .in_flight
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let map = guard.get_or_insert_with(HashMap::new);
            match map.get(&key) {
                Some(shared) => (Arc::clone(shared), false),
                None => {
                    let shared: Shared<V> = Arc::new((Mutex::new(Slot::Waiting), Condvar::new()));
                    map.insert(key.clone(), Arc::clone(&shared));
                    (shared, true)
                }
            }
        };
        if !leader {
            let (lock, ready) = &*shared;
            let mut slot = lock.lock().unwrap_or_else(PoisonError::into_inner);
            loop {
                match &*slot {
                    Slot::Done(value) => return value.clone(),
                    Slot::Abandoned => break,
                    Slot::Waiting => {
                        slot = ready.wait(slot).unwrap_or_else(PoisonError::into_inner);
                    }
                }
            }
            drop(slot);
            return read();
        }
        // The leader: publish whatever happens, so no waiter is left behind.
        let mut finish = Finish {
            owner: self,
            key,
            shared,
            value: None,
        };
        let value = read();
        finish.value = Some(value.clone());
        value
    }
}

struct Finish<'a, K: Eq + Hash, V> {
    owner: &'a SingleFlight<K, V>,
    key: K,
    shared: Shared<V>,
    value: Option<V>,
}

impl<K: Eq + Hash, V> Drop for Finish<'_, K, V> {
    fn drop(&mut self) {
        if let Some(map) = self
            .owner
            .in_flight
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
        {
            map.remove(&self.key);
        }
        let (lock, ready) = &*self.shared;
        *lock.lock().unwrap_or_else(PoisonError::into_inner) = match self.value.take() {
            Some(value) => Slot::Done(value),
            None => Slot::Abandoned,
        };
        ready.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[test]
    fn askers_arriving_together_share_one_read() {
        static FLIGHT: SingleFlight<u32, u64> = SingleFlight::new();
        let reads = Arc::new(AtomicUsize::new(0));
        let answers: Vec<u64> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..3)
                .map(|_| {
                    let reads = Arc::clone(&reads);
                    scope.spawn(move || {
                        FLIGHT.run(7, || {
                            reads.fetch_add(1, Ordering::SeqCst);
                            std::thread::sleep(Duration::from_millis(150));
                            42
                        })
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().expect("joins")).collect()
        });
        assert_eq!(answers, vec![42, 42, 42]);
        assert_eq!(reads.load(Ordering::SeqCst), 1, "one request for three askers");
    }

    #[test]
    fn a_later_asker_reads_again() {
        static FLIGHT: SingleFlight<u32, u64> = SingleFlight::new();
        assert_eq!(FLIGHT.run(1, || 1), 1);
        assert_eq!(FLIGHT.run(1, || 2), 2, "nothing is cached here");
    }
}

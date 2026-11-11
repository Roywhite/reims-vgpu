//! Windows MMIO wakes the control consumer through a native condition variable.

use parking_lot::{Condvar, Mutex};

#[derive(Debug, Default)]
struct State {
    pending: bool,
    #[cfg(test)]
    sleeping: bool,
}

/// One coalesced pending wake. Publishing before sleeping is retained, and
/// testing the parked state cannot consume a wake needed by the consumer.
#[derive(Debug)]
pub(crate) struct Wake {
    state: Mutex<State>,
    changed: Condvar,
}

impl Wake {
    pub(crate) fn new() -> Option<Self> {
        Some(Self {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
        })
    }

    pub(crate) fn wake(&self) {
        let mut state = self.state.lock();
        state.pending = true;
        self.changed.notify_one();
    }

    pub(crate) fn sleep(&self, doorbell: Option<i32>) {
        assert!(
            doorbell.is_none(),
            "Windows control wake expects MMIO, not a Unix fd"
        );
        let mut state = self.state.lock();
        while !state.pending {
            #[cfg(test)]
            {
                state.sleeping = true;
            }
            self.changed.wait(&mut state);
        }
        #[cfg(test)]
        {
            state.sleeping = false;
        }
        state.pending = false;
    }

    #[cfg(test)]
    pub(crate) fn parked(&self, doorbell: Option<i32>) -> bool {
        assert!(doorbell.is_none());
        let state = self.state.lock();
        state.sleeping && !state.pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// An early wake remains pending, and querying sleep never steals its wake.
    #[test]
    fn a_wake_before_or_during_sleep_is_delivered() {
        let wake = Arc::new(Wake::new().unwrap());
        wake.wake();
        wake.sleep(None);
        let sleeper = Arc::clone(&wake);
        let thread = std::thread::spawn(move || sleeper.sleep(None));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !wake.parked(None) {
            assert!(
                std::time::Instant::now() < deadline,
                "consumer never parked"
            );
            std::thread::yield_now();
        }
        assert!(wake.parked(None));
        wake.wake();
        thread.join().unwrap();
        assert!(!wake.parked(None));
    }
}

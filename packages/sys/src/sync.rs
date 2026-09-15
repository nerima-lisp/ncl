use std::sync::{Condvar as StdCondvar, Mutex as StdMutex, MutexGuard};
use std::time::Duration;

/// A poison-tolerant mutex used by runtime subsystems.
#[derive(Debug)]
pub struct Mutex<T>(StdMutex<T>);
impl<T> Mutex<T> {
    pub const fn new(value: T) -> Self {
        Self(StdMutex::new(value))
    }
    pub fn lock(&self) -> MutexGuard<'_, T> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
/// A condition variable paired with [`Mutex`].
#[derive(Debug, Default)]
pub struct Condvar(StdCondvar);
impl Condvar {
    pub fn wait<'a, T>(&self, guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
        self.0
            .wait(guard)
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    pub fn wait_timeout<'a, T>(
        &self,
        guard: MutexGuard<'a, T>,
        timeout: Duration,
    ) -> (MutexGuard<'a, T>, bool) {
        let (guard, result) = self
            .0
            .wait_timeout(guard, timeout)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (guard, result.timed_out())
    }
    pub fn notify_one(&self) {
        self.0.notify_one();
    }
    pub fn notify_all(&self) {
        self.0.notify_all();
    }
}
/// Counting semaphore for bounded runtime resources.
#[derive(Debug)]
pub struct Semaphore {
    state: Mutex<usize>,
    available: Condvar,
}
impl Semaphore {
    #[must_use]
    pub fn new(count: usize) -> Self {
        Self {
            state: Mutex::new(count),
            available: Condvar::default(),
        }
    }
    pub fn acquire(&self) {
        let mut count = self.state.lock();
        while *count == 0 {
            count = self.available.wait(count);
        }
        *count -= 1;
    }
    pub fn release(&self) {
        *self.state.lock() += 1;
        self.available.notify_one();
    }
}
/// Wakeable wait queue used by interruptible blocking operations.
#[derive(Debug)]
pub struct WaitQueue {
    state: Mutex<usize>,
    available: Condvar,
}
impl Default for WaitQueue {
    fn default() -> Self {
        Self {
            state: Mutex::new(0),
            available: Condvar::default(),
        }
    }
}
impl WaitQueue {
    pub fn wait(&self) {
        let mut ready = self.state.lock();
        while *ready == 0 {
            ready = self.available.wait(ready);
        }
        *ready -= 1;
    }
    pub fn wake_one(&self) {
        let mut state = self.state.lock();
        if *state != usize::MAX {
            *state += 1;
        }
        drop(state);
        self.available.notify_one();
    }
    pub fn wake_all(&self) {
        *self.state.lock() = usize::MAX;
        self.available.notify_all();
    }
}

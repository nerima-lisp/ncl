use std::sync::{Condvar as StdCondvar, Mutex as StdMutex, MutexGuard};
use std::time::Duration;

/// A poison-tolerant mutex used by runtime subsystems.
#[derive(Debug)]
pub struct Mutex<T>(StdMutex<T>);
impl<T> Mutex<T> {
    /// Construct a mutex containing `value`.
    pub const fn new(value: T) -> Self {
        Self(StdMutex::new(value))
    }
    /// Lock the mutex, recovering its value if a previous holder panicked.
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
    /// Sleep until notified and recover a poisoned mutex guard.
    pub fn wait<'a, T>(&self, guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
        self.0
            .wait(guard)
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    /// Sleep until notified or `timeout` expires; return whether it timed out.
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
    /// Wake one waiter.
    pub fn notify_one(&self) {
        self.0.notify_one();
    }
    /// Wake all waiters.
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
    /// Construct a semaphore with `count` available permits.
    #[must_use]
    pub fn new(count: usize) -> Self {
        Self {
            state: Mutex::new(count),
            available: Condvar::default(),
        }
    }
    /// Wait for and consume one permit.
    pub fn acquire(&self) {
        let mut count = self.state.lock();
        while *count == 0 {
            count = self.available.wait(count);
        }
        *count -= 1;
    }
    /// Return one permit and wake a waiter.
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
    /// Wait for a wake token.
    pub fn wait(&self) {
        let mut ready = self.state.lock();
        while *ready == 0 {
            ready = self.available.wait(ready);
        }
        *ready -= 1;
    }
    /// Add one wake token and wake one waiter.
    pub fn wake_one(&self) {
        let mut state = self.state.lock();
        if *state != usize::MAX {
            *state += 1;
        }
        drop(state);
        self.available.notify_one();
    }
    /// Wake every current and future waiter until the queue is reset.
    pub fn wake_all(&self) {
        *self.state.lock() = usize::MAX;
        self.available.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::WaitQueue;

    #[test]
    fn wake_one_does_not_overflow_after_wake_all() {
        let queue = WaitQueue::default();

        queue.wake_all();
        queue.wake_one();

        assert_eq!(*queue.state.lock(), usize::MAX);
    }
}

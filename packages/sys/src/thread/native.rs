use super::{NativeState, SafepointState, Thread};
use core::ops::{Deref, DerefMut};

#[cfg(test)]
std::thread_local! {
    static TEST_NATIVE_WAIT_SIGNAL: std::cell::RefCell<Option<std::sync::mpsc::Sender<()>>> =
        const { std::cell::RefCell::new(None) };
}

pub struct LispStateGuard<'a> {
    thread: &'a mut Thread,
    restore_native: bool,
}

impl Deref for LispStateGuard<'_> {
    type Target = Thread;

    fn deref(&self) -> &Self::Target {
        self.thread
    }
}

impl DerefMut for LispStateGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.thread
    }
}

impl Drop for LispStateGuard<'_> {
    fn drop(&mut self) {
        if self.restore_native {
            self.thread.enter_native();
        }
    }
}

impl Thread {
    const fn transition_to_lisp_if_idle(&mut self, stop_world: &crate::stw::StopWorld) -> bool {
        if stop_world.requested {
            return false;
        }
        self.native = NativeState::Lisp;
        self.state = SafepointState::Running;
        true
    }

    pub(crate) fn enter_lisp_state(&mut self) -> LispStateGuard<'_> {
        let restore_native = self.native == NativeState::Native;
        if restore_native {
            self.leave_native();
        }
        LispStateGuard {
            thread: self,
            restore_native,
        }
    }

    /// Enter the native runtime state.
    pub fn enter_native(&mut self) {
        if self.native == NativeState::Native {
            return;
        }
        let Some(heap) = self.heap else {
            self.native = NativeState::Native;
            self.state = SafepointState::Safe;
            return;
        };
        // SAFETY: register_thread installs this pointer until unregister_thread.
        let heap = unsafe { &*heap };
        loop {
            let stop_world = heap
                .stop_world
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !stop_world.requested {
                // Native threads are outside the mutator quorum, so publish their stack first.
                self.publish_snapshot();
                self.native = NativeState::Native;
                self.state = SafepointState::Safe;
                self.publish_gc_state(stop_world.epoch);
                drop(stop_world);
                heap.stop_world_ready.notify_all();
                return;
            }
            drop(stop_world);
            self.poll_safepoint();
        }
    }

    /// Leave the native runtime state and deliver a pending poll.
    pub fn leave_native(&mut self) {
        if let Some(heap) = self.heap {
            // SAFETY: register_thread installs this pointer until unregister_thread.
            let heap = unsafe { &*heap };
            if self.native == NativeState::Native {
                let mut stop_world = heap
                    .stop_world
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if !self.transition_to_lisp_if_idle(&stop_world) {
                    // Native code may have changed callee-saved roots since entering Native.
                    self.publish_snapshot();
                    #[cfg(test)]
                    TEST_NATIVE_WAIT_SIGNAL.with(|signal| {
                        if let Some(signal) = signal.borrow().as_ref() {
                            let _ = signal.send(());
                        }
                    });
                    heap.stop_world_ready.notify_all();
                    loop {
                        stop_world = heap
                            .stop_world_ready
                            .wait(stop_world)
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        if self.transition_to_lisp_if_idle(&stop_world) {
                            break;
                        }
                    }
                }
                drop(stop_world);
                heap.stop_world_ready.notify_all();
            } else {
                self.state = SafepointState::Running;
            }
        } else {
            self.native = NativeState::Lisp;
            self.state = SafepointState::Running;
        }
        self.poll_safepoint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, mpsc};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn native_transition_refuses_an_active_collection() {
        let heap = crate::Heap::new(crate::HeapConfig::default());
        let mut thread = Thread::new();
        assert_eq!(heap.register_thread(&mut thread), Ok(()));
        thread.enter_native();
        let gc_state = thread.gc_state_handle();
        let published = gc_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert!(published.is_native);
        assert_eq!(published.stack_bounds, thread.stack_bounds);
        assert_eq!(published.stack_scan_start, thread.stack_scan_start);
        assert!(
            published
                .stack_bounds
                .zip(published.stack_scan_start)
                .is_some_and(
                    |((start, end), stack_pointer)| start <= stack_pointer && stack_pointer < end
                )
        );
        drop(published);

        let mut stop_world = heap
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        stop_world.requested = true;
        assert!(!thread.transition_to_lisp_if_idle(&stop_world));
        assert_eq!(thread.native_state(), NativeState::Native);
        assert_eq!(thread.safepoint_state(), SafepointState::Safe);

        stop_world.requested = false;
        assert!(thread.transition_to_lisp_if_idle(&stop_world));
        drop(stop_world);
        heap.unregister_thread(&thread);
        assert_eq!(thread.native_state(), NativeState::Lisp);
        assert_eq!(thread.safepoint_state(), SafepointState::Running);
    }

    #[test]
    fn registering_a_native_thread_publishes_its_stack() {
        let heap = crate::Heap::new(crate::HeapConfig::default());
        let mut thread = Thread::new();
        thread.enter_native();
        assert_eq!(heap.register_thread(&mut thread), Ok(()));
        assert!(thread.stack_bounds.is_some());
        heap.unregister_thread(&thread);
    }

    #[test]
    fn native_root_set_mutation_waits_until_collection_request_ends() {
        let heap = Arc::new(crate::Heap::new(crate::HeapConfig::default()));
        let (ready_sender, ready_receiver) = mpsc::channel();
        let (mutate_sender, mutate_receiver) = mpsc::channel();
        let (waiting_sender, waiting_receiver) = mpsc::channel();
        let (result_sender, result_receiver) = mpsc::channel();
        let worker_heap = Arc::clone(&heap);
        let worker = thread::spawn(move || {
            let mut thread = Thread::new();
            assert_eq!(worker_heap.register_thread(&mut thread), Ok(()));
            thread.enter_native();
            ready_sender.send(()).ok();
            assert!(mutate_receiver.recv().is_ok());

            TEST_NATIVE_WAIT_SIGNAL.with(|signal| {
                *signal.borrow_mut() = Some(waiting_sender);
            });
            let mut value = crate::Word::fixnum(17);
            let _token = crate::ThreadAccess::new(&mut thread)
                .register_root_set(std::slice::from_mut(&mut value));
            result_sender
                .send((thread.roots.len(), thread.native_state()))
                .ok();
            worker_heap.unregister_thread(&thread);
        });

        assert!(ready_receiver.recv().is_ok());
        let mut stop_world = heap
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        stop_world.requested = true;
        stop_world.epoch = stop_world.epoch.wrapping_add(1).max(1);
        drop(stop_world);
        assert!(mutate_sender.send(()).is_ok());

        waiting_receiver
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|_| panic!("native root mutation should wait at the safepoint"));
        let mut stop_world = heap
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert!(stop_world.requested);
        assert!(matches!(
            result_receiver.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        stop_world.requested = false;
        drop(stop_world);
        heap.stop_world_ready.notify_all();

        let (roots_len, native_state) = result_receiver
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|_| panic!("root mutation should complete after the request"));
        assert_eq!(roots_len, 1);
        assert_eq!(native_state, NativeState::Native);
        assert!(worker.join().is_ok());
    }
}

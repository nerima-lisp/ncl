use super::{NativeState, SafepointState, Thread};
use core::ops::{Deref, DerefMut};

pub(crate) struct LispStateGuard<'a> {
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
    fn transition_to_lisp_if_idle(&mut self, stop_world: &crate::stw::StopWorld) -> bool {
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
                while !self.transition_to_lisp_if_idle(&stop_world) {
                    stop_world = heap
                        .stop_world_ready
                        .wait(stop_world)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
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

    #[test]
    fn native_transition_refuses_an_active_collection() {
        let heap = crate::Heap::new(crate::HeapConfig::default());
        let mut thread = Thread::new();
        assert_eq!(heap.register_thread(&mut thread), Ok(()));
        thread.enter_native();

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
}

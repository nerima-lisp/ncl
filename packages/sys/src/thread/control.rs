//! Machine-visible accessors for the non-local exit control fields.
//!
//! These fields (`pending`, `handler`, `cleanup`, `catch`, `mv`, `mv_count`)
//! are read directly by generated code through [`super::thread_layout`], and
//! are also read and written from Rust runtime functions (`ncl-runtime`)
//! that implement `catch`/`throw`/`unwind-protect`/`progv`. Keeping the
//! accessors here (rather than inline in `thread.rs`) keeps that file under
//! the project's line budget.

use super::{MULTIPLE_VALUE_AREA_WORDS, Thread};
use crate::word::Word;

/// Source whose Rust-side error payload is pending for this thread.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RuntimeErrorSource {
    /// An object-layer runtime error.
    Object = 0,
    /// A typed Lisp condition.
    Lisp = 1,
    /// A direct native runtime error.
    Native = 2,
}

impl RuntimeErrorSource {
    const fn mask(self) -> u64 {
        match self {
            Self::Object => 1,
            Self::Lisp => 1 << 1,
            Self::Native => 1 << 2,
        }
    }
}

/// Associates a payload type with the source flag consumed by its Rust boundary.
///
/// Implement this for payload types handled through the matching source slot.
pub trait RuntimeErrorPayloadKind: Copy {
    /// Source flag set while this payload is stored on a thread.
    const SOURCE: RuntimeErrorSource;
}

/// Stores a payload whose type determines which source flag is updated.
#[derive(Debug)]
pub struct RuntimeErrorPayloadSlot<T: RuntimeErrorPayloadKind> {
    payload: Option<T>,
}

impl<T: RuntimeErrorPayloadKind> RuntimeErrorPayloadSlot<T> {
    /// Create an empty payload slot.
    #[must_use]
    pub const fn new() -> Self {
        Self { payload: None }
    }

    /// Whether this slot contains no payload.
    #[must_use]
    pub const fn is_none(&self) -> bool {
        self.payload.is_none()
    }
}

impl<T: RuntimeErrorPayloadKind> Default for RuntimeErrorPayloadSlot<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl Thread {
    /// Record whether a Rust-side runtime error payload is pending.
    pub(crate) const fn set_runtime_error_pending(
        &mut self,
        source: RuntimeErrorSource,
        pending: bool,
    ) {
        let mask = source.mask();
        self.runtime_error_flags = if pending {
            self.runtime_error_flags | mask
        } else {
            self.runtime_error_flags & !mask
        };
    }

    /// Store a typed payload and mark its source pending in one operation.
    ///
    /// ```compile_fail
    /// let mut thread = ncl_sys::Thread::new();
    /// let mut payload = None::<()>;
    /// thread.record_runtime_error_payload(&mut payload, ());
    /// ```
    pub const fn record_runtime_error_payload<T: RuntimeErrorPayloadKind>(
        &mut self,
        slot: &mut RuntimeErrorPayloadSlot<T>,
        payload: T,
    ) {
        slot.payload = Some(payload);
        self.set_runtime_error_pending(T::SOURCE, true);
    }

    /// Take a typed payload and clear its source flag, including for an empty slot.
    ///
    /// ```compile_fail
    /// let mut thread = ncl_sys::Thread::new();
    /// let mut payload = None::<()>;
    /// let _ = thread.take_runtime_error_payload(&mut payload);
    /// ```
    pub const fn take_runtime_error_payload<T: RuntimeErrorPayloadKind>(
        &mut self,
        slot: &mut RuntimeErrorPayloadSlot<T>,
    ) -> Option<T> {
        let payload = slot.payload.take();
        self.set_runtime_error_pending(T::SOURCE, false);
        payload
    }

    /// Whether any Rust-side runtime error payload is pending.
    #[must_use]
    pub const fn runtime_error_pending(&self) -> bool {
        self.runtime_error_flags != 0
    }

    /// Whether a non-local exit is currently propagating through native code.
    #[must_use]
    pub const fn pending(&self) -> bool {
        self.pending != 0
    }

    /// Record whether a non-local exit is currently propagating.
    pub const fn set_pending(&mut self, value: bool) {
        self.pending = if value { 1 } else { 0 };
    }

    /// Return the number of live words in the multiple-value return area.
    #[must_use]
    pub const fn mv_count(&self) -> usize {
        self.mv_count
    }

    /// Record the number of live words in the multiple-value return area.
    ///
    /// The count is clamped to the area's fixed capacity.
    pub const fn set_mv_count(&mut self, count: usize) {
        self.mv_count = if count > MULTIPLE_VALUE_AREA_WORDS {
            MULTIPLE_VALUE_AREA_WORDS
        } else {
            count
        };
    }

    /// Overwrite the leading words of the multiple-value return area and
    /// record how many were written.
    ///
    /// Extra capacity beyond `values` is left untouched; readers must use
    /// [`Self::mv_count`] rather than assuming the whole area is live.
    pub fn set_multiple_value_area(&mut self, values: &[Word]) {
        let count = values.len().min(MULTIPLE_VALUE_AREA_WORDS);
        for (slot, value) in self.mv.iter_mut().zip(values.iter()).take(count) {
            *slot = *value;
        }
        self.set_mv_count(count);
    }

    /// Number of currently active `catch` frames.
    #[must_use]
    pub const fn catch_depth(&self) -> usize {
        self.catch
    }

    /// Number of currently active `unwind-protect` frames.
    #[must_use]
    pub const fn cleanup_depth(&self) -> usize {
        self.cleanup
    }

    /// Number of currently active dynamic-extent handler frames of any kind.
    #[must_use]
    pub const fn handler_depth(&self) -> usize {
        self.handler
    }

    /// Push one dynamic-extent frame of the given kind onto the depth
    /// counters exposed to generated code.
    pub const fn push_control_depth(&mut self, kind: ControlFrameKind) {
        self.handler = self.handler.saturating_add(1);
        match kind {
            ControlFrameKind::Catch => self.catch = self.catch.saturating_add(1),
            ControlFrameKind::UnwindProtect => self.cleanup = self.cleanup.saturating_add(1),
            ControlFrameKind::Progv => {}
        }
    }

    /// Pop one dynamic-extent frame of the given kind.
    pub const fn pop_control_depth(&mut self, kind: ControlFrameKind) {
        self.handler = self.handler.saturating_sub(1);
        match kind {
            ControlFrameKind::Catch => self.catch = self.catch.saturating_sub(1),
            ControlFrameKind::UnwindProtect => self.cleanup = self.cleanup.saturating_sub(1),
            ControlFrameKind::Progv => {}
        }
    }
}

/// The three dynamic-extent frame kinds tracked by [`Thread`]'s depth
/// counters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlFrameKind {
    /// A `catch` frame, matched by tag identity.
    Catch,
    /// An `unwind-protect` frame, always run when unwound past.
    UnwindProtect,
    /// A `progv` frame, unbound when unwound past.
    Progv,
}

#[cfg(test)]
mod tests {
    use super::{
        ControlFrameKind, RuntimeErrorPayloadKind, RuntimeErrorPayloadSlot, RuntimeErrorSource,
        Thread,
    };
    use crate::word::Word;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct ObjectPayload;

    impl RuntimeErrorPayloadKind for ObjectPayload {
        const SOURCE: RuntimeErrorSource = RuntimeErrorSource::Object;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct LispPayload;

    impl RuntimeErrorPayloadKind for LispPayload {
        const SOURCE: RuntimeErrorSource = RuntimeErrorSource::Lisp;
    }

    #[test]
    fn pending_flag_round_trips() {
        let mut thread = Thread::new();
        assert!(!thread.pending());
        thread.set_pending(true);
        assert!(thread.pending());
        thread.set_pending(false);
        assert!(!thread.pending());
    }

    #[test]
    fn runtime_error_sources_are_independent_of_each_other_and_pending() {
        let mut thread = Thread::new();
        assert!(!thread.runtime_error_pending());
        assert!(!thread.pending());
        thread.set_runtime_error_pending(RuntimeErrorSource::Object, true);
        thread.set_runtime_error_pending(RuntimeErrorSource::Native, true);
        thread.set_pending(true);
        thread.set_runtime_error_pending(RuntimeErrorSource::Object, false);
        assert!(thread.runtime_error_pending());
        assert!(thread.pending());
        thread.set_runtime_error_pending(RuntimeErrorSource::Native, false);
        assert!(!thread.runtime_error_pending());
        assert!(thread.pending());
    }

    #[test]
    fn taking_empty_runtime_error_payload_clears_its_source_flag() {
        fn check<T: RuntimeErrorPayloadKind>() {
            let mut thread = Thread::new();
            let mut payload = RuntimeErrorPayloadSlot::<T>::new();
            thread.set_runtime_error_pending(T::SOURCE, true);
            assert!(thread.take_runtime_error_payload(&mut payload).is_none());
            assert!(!thread.runtime_error_pending());
        }

        check::<ObjectPayload>();
        check::<LispPayload>();
    }

    #[test]
    fn typed_runtime_error_payload_sets_and_clears_its_source_flag() {
        let mut thread = Thread::new();
        let mut object = RuntimeErrorPayloadSlot::<ObjectPayload>::new();
        let mut lisp = RuntimeErrorPayloadSlot::<LispPayload>::new();

        thread.record_runtime_error_payload(&mut object, ObjectPayload);
        thread.record_runtime_error_payload(&mut lisp, LispPayload);
        assert!(thread.runtime_error_pending());
        assert_eq!(
            thread.take_runtime_error_payload(&mut object),
            Some(ObjectPayload)
        );
        assert!(thread.runtime_error_pending());
        assert_eq!(
            thread.take_runtime_error_payload(&mut lisp),
            Some(LispPayload)
        );
        assert!(!thread.runtime_error_pending());
    }

    #[test]
    fn taking_empty_native_error_clears_its_source_flag() {
        let mut thread = Thread::new();
        thread.set_runtime_error_pending(RuntimeErrorSource::Native, true);
        assert_eq!(thread.take_native_error(), None);
        assert!(!thread.runtime_error_pending());
    }

    #[test]
    fn multiple_value_area_records_the_written_count() {
        let mut thread = Thread::new();
        thread.set_multiple_value_area(&[Word::fixnum(1), Word::fixnum(2)]);
        assert_eq!(thread.mv_count(), 2);
        assert_eq!(thread.multiple_values()[0], Word::fixnum(1));
        assert_eq!(thread.multiple_values()[1], Word::fixnum(2));
    }

    #[test]
    fn control_depth_counters_track_pushes_and_pops() {
        let mut thread = Thread::new();
        thread.push_control_depth(ControlFrameKind::Catch);
        thread.push_control_depth(ControlFrameKind::UnwindProtect);
        assert_eq!(thread.catch_depth(), 1);
        assert_eq!(thread.cleanup_depth(), 1);
        assert_eq!(thread.handler_depth(), 2);
        thread.pop_control_depth(ControlFrameKind::UnwindProtect);
        assert_eq!(thread.cleanup_depth(), 0);
        assert_eq!(thread.handler_depth(), 1);
    }
}

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

impl Thread {
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
        // check-added-lines: allow(index) `count` is clamped to both slice lengths on the line above; both slices always contain at least `count` elements.
        self.mv[..count].copy_from_slice(&values[..count]);
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
    use super::{ControlFrameKind, Thread};
    use crate::word::Word;

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

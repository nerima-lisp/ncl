use super::Thread;

/// Native offsets consumed by the code generator when addressing a thread context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThreadLayout {
    /// Offset of the TLAB allocation cursor.
    pub tlab_bump: usize,
    /// Offset of the TLAB allocation limit.
    pub tlab_limit: usize,
    /// Offset of the eight-byte safepoint request word read by generated code.
    pub safepoint_request: usize,
    /// Offset of the eight-byte pending interrupt word.
    pub pending: usize,
    /// Offset of the multiple-value return vector.
    pub mv: usize,
    /// Offset of the handler chain.
    pub handler: usize,
    /// Offset of the cleanup chain.
    pub cleanup: usize,
    /// Offset of the catch chain.
    pub catch: usize,
    /// Offset of the multiple-value count word.
    pub mv_count: usize,
    /// Offset of the Rust-side runtime-error source flags.
    pub runtime_error_flags: usize,
}

/// Return byte offsets for the machine-visible part of [`Thread`].
#[must_use]
pub const fn thread_layout() -> ThreadLayout {
    ThreadLayout {
        tlab_bump: std::mem::offset_of!(Thread, tlab_bump),
        tlab_limit: std::mem::offset_of!(Thread, tlab_limit),
        safepoint_request: std::mem::offset_of!(Thread, safepoint_request),
        pending: std::mem::offset_of!(Thread, pending),
        mv: std::mem::offset_of!(Thread, mv),
        handler: std::mem::offset_of!(Thread, handler),
        cleanup: std::mem::offset_of!(Thread, cleanup),
        catch: std::mem::offset_of!(Thread, catch),
        mv_count: std::mem::offset_of!(Thread, mv_count),
        runtime_error_flags: std::mem::offset_of!(Thread, runtime_error_flags),
    }
}

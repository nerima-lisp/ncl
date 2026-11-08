use crate::Thread;

/// Configure the machine-visible TLAB range used by generated `AArch64` code.
pub const fn set_tlab(thread: &mut Thread, bump: usize, limit: usize) {
    thread.tlab_bump = bump;
    thread.tlab_limit = limit;
    thread.header_tlab_start = None;
    thread.header_tlab_page = None;
}

/// Return the machine-visible TLAB bump pointer.
#[must_use]
pub const fn tlab_bump(thread: &Thread) -> usize {
    thread.tlab_bump
}

/// Configure the machine-visible cons TLAB range.
pub const fn set_cons_tlab(thread: &mut Thread, bump: usize, limit: usize) {
    thread.cons_tlab_bump = bump;
    thread.cons_tlab_limit = limit;
    thread.cons_tlab_start = None;
    thread.cons_tlab_page = None;
}

use crate::Thread;

/// Configure the machine-visible TLAB range used by generated `AArch64` code.
pub const fn set_tlab(thread: &mut Thread, bump: usize, limit: usize) {
    thread.tlab_bump = bump;
    thread.tlab_limit = limit;
}

/// Return the machine-visible TLAB bump pointer.
#[must_use]
pub const fn tlab_bump(thread: &Thread) -> usize {
    thread.tlab_bump
}

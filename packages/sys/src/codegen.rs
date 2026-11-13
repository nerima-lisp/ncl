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

#[cfg(test)]
mod tests {
    use super::set_cons_tlab;
    use crate::Thread;
    use std::ptr::NonNull;

    #[test]
    fn setting_cons_tlab_discards_the_previous_page() {
        let mut thread = Thread::new();
        thread.cons_tlab_start = Some(NonNull::dangling());
        thread.cons_tlab_page = Some(3);

        set_cons_tlab(&mut thread, 16, 64);

        assert_eq!(thread.cons_tlab_bump, 16);
        assert_eq!(thread.cons_tlab_limit, 64);
        assert_eq!(thread.cons_tlab_start, None);
        assert_eq!(thread.cons_tlab_page, None);
    }
}

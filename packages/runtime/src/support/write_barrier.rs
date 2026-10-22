use std::ptr::NonNull;

use ncl_object::Word;
use ncl_sys::Thread;

pub(super) extern "C" fn native_write_barrier(thread: NonNull<Thread>, slot: usize, owner: Word) {
    ncl_sys::write_barrier(thread, owner, slot);
}

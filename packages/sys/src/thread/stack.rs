#[cfg(target_os = "linux")]
use std::ptr;

#[cfg(target_os = "macos")]
pub(super) fn current_stack_bounds() -> Option<(usize, usize)> {
    // SAFETY: pthread_self identifies the calling thread and both APIs return its live stack extent.
    unsafe {
        let thread = crate::os::declarations::pthread_self();
        let top = crate::os::declarations::pthread_get_stackaddr_np(thread).addr();
        let size = crate::os::declarations::pthread_get_stacksize_np(thread);
        top.checked_sub(size).map(|start| (start, top))
    }
}

#[cfg(target_os = "linux")]
pub(super) fn current_stack_bounds() -> Option<(usize, usize)> {
    let mut attr = [0_u8; 128];
    let mut start = ptr::null_mut();
    let mut size = 0;
    // SAFETY: pthread attributes are written to platform ABI storage and destroyed after use.
    let result = unsafe {
        let thread = crate::os::declarations::pthread_self();
        let result = crate::os::declarations::pthread_getattr_np(thread, attr.as_mut_ptr().cast());
        if result == 0 {
            let result = crate::os::declarations::pthread_attr_getstack(
                attr.as_ptr().cast(),
                &raw mut start,
                &raw mut size,
            );
            let _ = crate::os::declarations::pthread_attr_destroy(attr.as_mut_ptr().cast());
            result
        } else {
            result
        }
    };
    (result == 0).then_some((start.addr(), start.addr().saturating_add(size)))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(super) const fn current_stack_bounds() -> Option<(usize, usize)> {
    None
}

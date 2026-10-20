//! Platform boundary declarations used by the runtime.

#[cfg(unix)]
/// C ABI declarations used by allocation, threading, and dynamic loading code.
pub mod declarations {
    use core::ffi::{c_char, c_int, c_void};
    // SAFETY: these declarations use the platform C ABI; all calls remain in this
    // crate and must satisfy each function's documented platform preconditions.
    unsafe extern "C" {
        /// POSIX anonymous memory mapping used for code space.
        pub fn mmap(
            addr: *mut c_void,
            len: usize,
            prot: c_int,
            flags: c_int,
            fd: c_int,
            offset: isize,
        ) -> *mut c_void;
        /// POSIX unmapping used when code space is dropped.
        pub fn munmap(addr: *mut c_void, len: usize) -> c_int;
        /// POSIX page-protection change used to publish code.
        #[cfg(any(target_os = "linux", test))]
        pub fn mprotect(addr: *mut c_void, len: usize, prot: c_int) -> c_int;
        /// Return the calling pthread identifier.
        pub fn pthread_self() -> usize;
        /// Open a dynamic library for symbol lookup.
        pub fn dlopen(path: *const c_char, flags: c_int) -> *mut c_void;
        /// Resolve a symbol from a dynamic library handle.
        pub fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
        /// Close a dynamic library handle.
        pub fn dlclose(handle: *mut c_void) -> c_int;
        /// Return the most recent dynamic-loader error.
        pub fn dlerror() -> *const c_char;
        /// Read bytes from a POSIX file descriptor.
        #[cfg(test)]
        pub fn read(fd: c_int, buf: *mut c_void, count: usize) -> isize;
        /// Open a POSIX path, optionally with a mode argument.
        #[cfg(test)]
        pub fn open(path: *const c_char, flags: c_int, ...) -> c_int;
        /// Close a POSIX file descriptor.
        #[cfg(test)]
        pub fn close(fd: c_int) -> c_int;
        /// Read a POSIX clock value.
        #[cfg(test)]
        pub fn clock_gettime(clock: c_int, time: *mut c_void) -> c_int;
    }
    #[cfg(target_os = "macos")]
    // SAFETY: these declarations match macOS pthread and cache-management C entry points.
    unsafe extern "C" {
        /// Return the macOS stack top for a pthread.
        pub fn pthread_get_stackaddr_np(thread: usize) -> *mut c_void;
        /// Return the macOS stack size for a pthread.
        pub fn pthread_get_stacksize_np(thread: usize) -> usize;
        /// Enable or disable macOS JIT write protection.
        pub fn pthread_jit_write_protect_np(enabled: c_int);
        /// Flush macOS instruction cache after writing JIT code.
        pub fn sys_icache_invalidate(start: *const c_void, len: usize);
    }
    #[cfg(target_os = "linux")]
    // SAFETY: these declarations match Linux pthread attribute C entry points.
    unsafe extern "C" {
        /// Read Linux pthread attributes for stack discovery.
        pub fn pthread_getattr_np(thread: usize, attr: *mut c_void) -> c_int;
        /// Extract a Linux pthread's stack range.
        pub fn pthread_attr_getstack(
            attr: *const c_void,
            stack: *mut *mut c_void,
            size: *mut usize,
        ) -> c_int;
        /// Release temporary Linux pthread attributes.
        pub fn pthread_attr_destroy(attr: *mut c_void) -> c_int;
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use core::ffi::c_void;
        #[cfg(target_os = "macos")]
        const MAP_ANONYMOUS: c_int = 0x1000;
        #[cfg(target_os = "linux")]
        const MAP_ANONYMOUS: c_int = 0x20;

        const MAP_PRIVATE: c_int = 0x02;
        const PROT_READ: c_int = 0x01;
        const PROT_WRITE: c_int = 0x02;
        const RTLD_NOW: c_int = 0x02;

        #[test]
        fn memory_mapping_can_be_written_protected_and_released() {
            let page_size = 4096;
            // SAFETY: the arguments request one private anonymous page with valid flags.
            let mapping = unsafe {
                mmap(
                    core::ptr::null_mut(),
                    page_size,
                    PROT_READ | PROT_WRITE,
                    MAP_PRIVATE | MAP_ANONYMOUS,
                    -1,
                    0,
                )
            };
            assert_ne!(mapping, (-1_isize) as *mut c_void);

            // SAFETY: mmap returned a valid writable page and the byte is within its mapping.
            unsafe { *(mapping.cast::<u8>()) = 0xa5 };
            // SAFETY: the byte remains within the live mapping.
            assert_eq!(unsafe { *(mapping.cast::<u8>()) }, 0xa5);
            // SAFETY: the address and length are the successful mmap result.
            assert_eq!(unsafe { mprotect(mapping, page_size, PROT_READ) }, 0);
            // SAFETY: the mapping has not been released or reused.
            assert_eq!(unsafe { munmap(mapping, page_size) }, 0);
        }

        #[test]
        fn file_io_and_process_clock_report_success() {
            let path = c"/dev/null";
            // SAFETY: the C string is NUL-terminated and the flags open an existing path read-only.
            let fd = unsafe { open(path.as_ptr(), 0, 0) };
            assert!(fd >= 0);

            let mut byte = 0_u8;
            // SAFETY: byte is valid for one-byte output and fd was successfully opened.
            assert_eq!(unsafe { read(fd, (&raw mut byte).cast(), 1) }, 0);
            // SAFETY: fd was returned by open and has not been closed.
            assert_eq!(unsafe { close(fd) }, 0);

            let mut time = [0_i64; 2];
            // SAFETY: time points to storage matching the POSIX timespec layout.
            assert_eq!(unsafe { clock_gettime(0, time.as_mut_ptr().cast()) }, 0);
            assert!(time[0] > 0);
            assert!(time[1] >= 0);
        }

        #[test]
        fn pthread_identity_is_stable_for_the_calling_thread() {
            // SAFETY: pthread_self has no pointer or lifetime preconditions.
            let first = unsafe { pthread_self() };
            // SAFETY: pthread_self has no pointer or lifetime preconditions.
            let second = unsafe { pthread_self() };
            assert_ne!(first, 0);
            assert_eq!(first, second);
        }

        #[test]
        fn dynamic_loader_reports_missing_library_and_resolves_symbols() {
            let missing = c"/definitely/missing/ncl-library.dylib";
            // SAFETY: the C string is NUL-terminated and names a deliberately missing library.
            assert!(unsafe { dlopen(missing.as_ptr(), RTLD_NOW) }.is_null());
            // SAFETY: dlerror reads the loader's thread-local error state.
            assert!(!unsafe { dlerror() }.is_null());
            // SAFETY: dlerror reads and clears the loader's thread-local error state.
            assert!(unsafe { dlerror() }.is_null());

            // SAFETY: a null path requests the current process handle.
            let handle = unsafe { dlopen(core::ptr::null(), RTLD_NOW) };
            assert!(!handle.is_null());
            let symbol = c"dlopen";
            // SAFETY: handle is a successful loader handle and symbol is NUL-terminated.
            assert!(!unsafe { dlsym(handle, symbol.as_ptr()) }.is_null());
        }
    }
}

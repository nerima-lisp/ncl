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

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod signal_control {
    #[cfg(target_os = "linux")]
    use core::ffi::c_long;
    use core::ffi::{c_int, c_void};
    use std::cell::RefCell;
    use std::io;

    #[cfg(target_os = "linux")]
    const SIG_CHLD: c_int = 17;
    #[cfg(target_os = "macos")]
    const SIG_CHLD: c_int = 20;
    const SIG_INT: c_int = 2;
    const SIG_PIPE: c_int = 13;
    const SIG_TERM: c_int = 15;
    const SIG_IGN: usize = 1;
    #[cfg(target_os = "linux")]
    const SIG_BLOCK: c_int = 0;
    #[cfg(target_os = "macos")]
    const SIG_BLOCK: c_int = 1;
    #[cfg(target_os = "linux")]
    const SIG_UNBLOCK: c_int = 1;
    #[cfg(target_os = "macos")]
    const SIG_UNBLOCK: c_int = 2;
    #[cfg(target_os = "linux")]
    const SS_DISABLE: c_int = 2;
    #[cfg(target_os = "macos")]
    const SS_DISABLE: c_int = 4;
    const SS_ONSTACK: c_int = 1;
    const ALT_SIGNAL_HANDLER_RESERVE: usize = 64 * 1024;

    #[cfg(target_os = "linux")]
    const LINUX_SC_MINSIGSTKSZ: c_int = 249;
    #[cfg(target_os = "linux")]
    const LINUX_SC_SIGSTKSZ: c_int = 250;
    #[cfg(target_os = "linux")]
    const LINUX_MINSIGSTKSZ_FALLBACK: usize = 2 * 1024;
    #[cfg(target_os = "linux")]
    const LINUX_SIGSTKSZ_FALLBACK: usize = 8 * 1024;
    #[cfg(target_os = "macos")]
    const MACOS_MINSIGSTKSZ: usize = 32 * 1024;
    #[cfg(target_os = "macos")]
    const MACOS_SIGSTKSZ: usize = 128 * 1024;

    #[cfg(target_os = "linux")]
    #[repr(C)]
    struct SigSet {
        _words: [usize; 16],
    }

    #[cfg(target_os = "macos")]
    #[repr(C)]
    struct SigSet {
        _bits: u32,
    }

    #[cfg(target_os = "linux")]
    #[repr(C)]
    struct SigAction {
        handler: usize,
        mask: SigSet,
        flags: c_int,
        _restorer: *mut c_void,
    }

    #[cfg(target_os = "macos")]
    #[repr(C)]
    struct SigAction {
        handler: usize,
        mask: SigSet,
        flags: c_int,
    }

    #[cfg(target_os = "linux")]
    #[repr(C)]
    struct AltSignalStack {
        sp: *mut c_void,
        flags: c_int,
        size: usize,
    }

    #[cfg(target_os = "macos")]
    #[repr(C)]
    struct AltSignalStack {
        sp: *mut c_void,
        size: usize,
        flags: c_int,
    }

    unsafe extern "C" {
        fn sigaction(signal: c_int, action: *const SigAction, old_action: *mut SigAction) -> c_int;
        fn sigaddset(set: *mut SigSet, signal: c_int) -> c_int;
        fn sigaltstack(stack: *const AltSignalStack, old_stack: *mut AltSignalStack) -> c_int;
        fn sigemptyset(set: *mut SigSet) -> c_int;
        fn sigwait(set: *const SigSet, signal: *mut c_int) -> c_int;
        fn getpagesize() -> c_int;
        #[cfg(target_os = "linux")]
        fn sysconf(name: c_int) -> c_long;
        fn pthread_sigmask(how: c_int, set: *const SigSet, old_set: *mut SigSet) -> c_int;
    }

    #[cfg(target_os = "linux")]
    unsafe extern "C" {
        fn __errno_location() -> *mut c_int;
    }

    #[cfg(target_os = "macos")]
    unsafe extern "C" {
        fn __error() -> *mut c_int;
    }

    fn last_os_error() -> io::Error {
        #[cfg(target_os = "linux")]
        // SAFETY: libc returns a pointer to the calling thread's errno slot.
        let code = unsafe { *__errno_location() };
        #[cfg(target_os = "macos")]
        // SAFETY: libc returns a pointer to the calling thread's errno slot.
        let code = unsafe { *__error() };
        io::Error::from_raw_os_error(code)
    }

    fn control_signal_set() -> io::Result<SigSet> {
        // SAFETY: zero is a valid initial representation before sigemptyset fills it.
        let mut set: SigSet = unsafe { core::mem::zeroed() };
        // SAFETY: set points to correctly sized storage for this target's sigset_t.
        if unsafe { sigemptyset(&raw mut set) } != 0 {
            return Err(last_os_error());
        }
        for signal in [SIG_INT, SIG_TERM, SIG_CHLD] {
            // SAFETY: set is initialized and signal is a valid target signal number.
            if unsafe { sigaddset(&raw mut set, signal) } != 0 {
                return Err(last_os_error());
            }
        }
        Ok(set)
    }

    #[cfg(target_os = "linux")]
    fn positive_sysconf(name: c_int, fallback: usize) -> usize {
        // SAFETY: sysconf accepts the Linux signal-stack configuration selectors.
        let value = unsafe { sysconf(name) };
        usize::try_from(value)
            .ok()
            .filter(|value| *value > 0)
            .unwrap_or(fallback)
    }

    fn required_alt_signal_stack_size() -> io::Result<usize> {
        #[cfg(target_os = "linux")]
        let (minimum, recommended) = (
            positive_sysconf(LINUX_SC_MINSIGSTKSZ, LINUX_MINSIGSTKSZ_FALLBACK),
            positive_sysconf(LINUX_SC_SIGSTKSZ, LINUX_SIGSTKSZ_FALLBACK),
        );
        #[cfg(target_os = "macos")]
        let (minimum, recommended) = (MACOS_MINSIGSTKSZ, MACOS_SIGSTKSZ);

        // SAFETY: getpagesize has no pointer preconditions.
        let page_size = unsafe { getpagesize() };
        let page_size = usize::try_from(page_size)
            .ok()
            .filter(|size| *size > 0)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid page size"))?;
        let base_size = minimum.max(recommended);
        let rounded_size = base_size
            .checked_add(page_size - 1)
            .map(|size| size / page_size * page_size)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "signal stack size overflow")
            })?;
        rounded_size
            .checked_add(ALT_SIGNAL_HANDLER_RESERVE)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "signal stack size overflow")
            })
    }

    fn current_alt_signal_stack() -> io::Result<AltSignalStack> {
        let mut current = AltSignalStack {
            sp: core::ptr::null_mut(),
            size: 0,
            flags: 0,
        };
        // SAFETY: current points to writable stack_t storage; null requests a query only.
        if unsafe { sigaltstack(core::ptr::null(), &raw mut current) } != 0 {
            return Err(last_os_error());
        }
        Ok(current)
    }

    struct AltSignalStackOwner {
        memory: Option<Box<[u8]>>,
    }

    impl Drop for AltSignalStackOwner {
        fn drop(&mut self) {
            let Some(memory) = self.memory.take() else {
                return;
            };
            let Ok(current) = current_alt_signal_stack() else {
                core::mem::forget(memory);
                return;
            };
            if current.sp != memory.as_ptr().cast_mut().cast() {
                return;
            }
            if current.flags & SS_ONSTACK != 0 {
                core::mem::forget(memory);
                return;
            }
            if current.flags & SS_DISABLE == 0 {
                let disabled = AltSignalStack {
                    sp: core::ptr::null_mut(),
                    size: 0,
                    flags: SS_DISABLE,
                };
                // SAFETY: the current thread owns this alternate stack, and it is not active.
                if unsafe { sigaltstack(&disabled, core::ptr::null_mut()) } != 0 {
                    core::mem::forget(memory);
                }
            }
        }
    }

    std::thread_local! {
        static ALT_SIGNAL_STACK: RefCell<AltSignalStackOwner> = const {
            RefCell::new(AltSignalStackOwner { memory: None })
        };
    }

    fn ensure_alt_signal_stack() -> io::Result<usize> {
        let required = required_alt_signal_stack_size()?;
        let current = current_alt_signal_stack()?;
        if current.flags & SS_DISABLE == 0 && current.size >= required {
            return Ok(current.size);
        }
        if current.flags & SS_ONSTACK != 0 {
            return Err(io::Error::from_raw_os_error(1));
        }

        ALT_SIGNAL_STACK
            .try_with(|owner| {
                let mut owner = owner.try_borrow_mut().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "alternate signal stack is borrowed",
                    )
                })?;
                let mut memory = vec![0_u8; required].into_boxed_slice();
                let stack = AltSignalStack {
                    sp: memory.as_mut_ptr().cast(),
                    size: memory.len(),
                    flags: 0,
                };
                // SAFETY: memory remains owned by thread-local storage for the stack's lifetime.
                if unsafe { sigaltstack(&stack, core::ptr::null_mut()) } != 0 {
                    return Err(last_os_error());
                }
                owner.memory = Some(memory);
                Ok(required)
            })
            .map_err(|_| io::Error::other("alternate signal stack TLS unavailable"))?
    }

    extern "C" fn sigchld_wait_handler(_: c_int) {}
    fn set_signal_disposition(signal: c_int, handler: usize) -> io::Result<()> {
        // SAFETY: zeroed storage is valid for sigaction's integer, pointer, and sigset fields.
        let mut action: SigAction = unsafe { core::mem::zeroed() };
        action.handler = handler;
        // SAFETY: action.mask has the target's sigset_t layout.
        if unsafe { sigemptyset(&raw mut action.mask) } != 0 {
            return Err(last_os_error());
        }
        // SAFETY: action has a valid handler and empty mask for this target's sigaction ABI.
        if unsafe { sigaction(signal, &raw const action, core::ptr::null_mut()) } != 0 {
            return Err(last_os_error());
        }
        Ok(())
    }

    impl crate::Thread {
        /// Ignore process-wide `SIGPIPE` so failed writes return `EPIPE` to normal code.
        pub fn ignore_sigpipe() -> io::Result<()> {
            set_signal_disposition(SIG_PIPE, SIG_IGN)
        }

        /// Install a no-op disposition so `SIGCHLD` can be consumed with `sigwait`.
        pub fn install_sigchld_wait_disposition() -> io::Result<()> {
            set_signal_disposition(SIG_CHLD, sigchld_wait_handler as *const () as usize)
        }

        /// Block `SIGINT`, `SIGTERM`, and `SIGCHLD` on the calling OS thread.
        pub fn block_signal_control_set() -> io::Result<()> {
            let set = control_signal_set()?;
            // SAFETY: set is initialized and old_set is null because its prior value is unused.
            let result =
                unsafe { pthread_sigmask(SIG_BLOCK, &raw const set, core::ptr::null_mut()) };
            if result != 0 {
                return Err(io::Error::from_raw_os_error(result));
            }
            Ok(())
        }

        /// Unblock the control-signal set on the calling OS thread.
        pub fn unblock_signal_control_set() -> io::Result<()> {
            let set = control_signal_set()?;
            // SAFETY: set is initialized and old_set is null because its prior value is unused.
            let result =
                unsafe { pthread_sigmask(SIG_UNBLOCK, &raw const set, core::ptr::null_mut()) };
            if result != 0 {
                return Err(io::Error::from_raw_os_error(result));
            }
            Ok(())
        }

        /// Wait for one blocked control signal and return its target signal number.
        pub fn wait_signal_control_set() -> io::Result<c_int> {
            let set = control_signal_set()?;
            loop {
                let mut signal = 0;
                // SAFETY: set is initialized and signal points to writable integer storage.
                let result = unsafe { sigwait(&raw const set, &raw mut signal) };
                if result == 0 {
                    return Ok(signal);
                }
                let error = io::Error::from_raw_os_error(result);
                if error.kind() != io::ErrorKind::Interrupted {
                    return Err(error);
                }
            }
        }

        /// Reuse or register a per-thread alternate signal stack of the contract size.
        pub fn ensure_alt_signal_stack() -> io::Result<usize> {
            ensure_alt_signal_stack()
        }
    }
}

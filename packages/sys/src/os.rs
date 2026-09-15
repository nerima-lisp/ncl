//! Platform boundary declarations used by the runtime.

#[cfg(unix)]
/// C ABI declarations used by allocation, threading, and dynamic loading code.
pub mod declarations {
    use core::ffi::{c_char, c_int, c_void};
    #[repr(C)]
    #[derive(Debug)]
    /// Opaque POSIX `struct stat` storage.
    pub struct Stat {
        _private: [u8; 0],
    }
    #[repr(C)]
    #[derive(Debug)]
    /// Opaque directory stream returned by POSIX `opendir`.
    pub struct Dirent {
        _private: [u8; 0],
    }
    #[repr(C)]
    #[derive(Debug)]
    /// Opaque pthread mutex storage.
    pub struct PthreadMutex {
        _private: [u8; 0],
    }
    #[repr(C)]
    #[derive(Debug)]
    /// Opaque pthread condition-variable storage.
    pub struct PthreadCond {
        _private: [u8; 0],
    }
    /// Opaque POSIX semaphore storage.
    pub type Sem = c_void;
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
        pub fn mprotect(addr: *mut c_void, len: usize, prot: c_int) -> c_int;
        /// Create a native runtime thread.
        pub fn pthread_create(
            thread: *mut usize,
            attr: *const c_void,
            start: Option<extern "C" fn(*mut c_void) -> *mut c_void>,
            arg: *mut c_void,
        ) -> c_int;
        /// Join a native runtime thread.
        pub fn pthread_join(thread: usize, value: *mut *mut c_void) -> c_int;
        /// Return the calling pthread identifier.
        pub fn pthread_self() -> usize;
        /// Open a dynamic library for symbol lookup.
        pub fn dlopen(path: *const c_char, flags: c_int) -> *mut c_void;
        /// Resolve a symbol from a dynamic library handle.
        pub fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
        /// Return the most recent dynamic-loader error.
        pub fn dlerror() -> *const c_char;
        /// Read bytes from a POSIX file descriptor.
        pub fn read(fd: c_int, buf: *mut c_void, count: usize) -> isize;
        /// Write bytes to a POSIX file descriptor.
        pub fn write(fd: c_int, buf: *const c_void, count: usize) -> isize;
        /// Open a POSIX path, optionally with a mode argument.
        pub fn open(path: *const c_char, flags: c_int, ...) -> c_int;
        /// Close a POSIX file descriptor.
        pub fn close(fd: c_int) -> c_int;
        /// Fill POSIX file metadata for a path.
        pub fn stat(path: *const c_char, output: *mut Stat) -> c_int;
        /// Open a POSIX directory stream.
        pub fn opendir(path: *const c_char) -> *mut Dirent;
        /// Read the next POSIX directory entry.
        pub fn readdir(dir: *mut Dirent) -> *mut Dirent;
        /// Close a POSIX directory stream.
        pub fn closedir(dir: *mut Dirent) -> c_int;
        /// Read a POSIX clock value.
        pub fn clock_gettime(clock: c_int, time: *mut c_void) -> c_int;
        /// Install a simple POSIX signal handler.
        pub fn signal(
            signum: c_int,
            handler: Option<extern "C" fn(c_int)>,
        ) -> Option<extern "C" fn(c_int)>;
        /// Install a POSIX signal action structure.
        pub fn sigaction(signum: c_int, action: *const c_void, old_action: *mut c_void) -> c_int;
        /// Initialize a pthread mutex.
        pub fn pthread_mutex_init(mutex: *mut PthreadMutex, attr: *const c_void) -> c_int;
        /// Destroy a pthread mutex.
        pub fn pthread_mutex_destroy(mutex: *mut PthreadMutex) -> c_int;
        /// Lock a pthread mutex.
        pub fn pthread_mutex_lock(mutex: *mut PthreadMutex) -> c_int;
        /// Unlock a pthread mutex.
        pub fn pthread_mutex_unlock(mutex: *mut PthreadMutex) -> c_int;
        /// Initialize a pthread condition variable.
        pub fn pthread_cond_init(cond: *mut PthreadCond, attr: *const c_void) -> c_int;
        /// Destroy a pthread condition variable.
        pub fn pthread_cond_destroy(cond: *mut PthreadCond) -> c_int;
        /// Wait on a pthread condition variable.
        pub fn pthread_cond_wait(cond: *mut PthreadCond, mutex: *mut PthreadMutex) -> c_int;
        /// Signal one pthread condition waiter.
        pub fn pthread_cond_signal(cond: *mut PthreadCond) -> c_int;
        /// Signal all pthread condition waiters.
        pub fn pthread_cond_broadcast(cond: *mut PthreadCond) -> c_int;
        /// Initialize a POSIX semaphore.
        pub fn sem_init(sem: *mut Sem, shared: c_int, value: u32) -> c_int;
        /// Destroy a POSIX semaphore.
        pub fn sem_destroy(sem: *mut Sem) -> c_int;
        /// Wait for a POSIX semaphore permit.
        pub fn sem_wait(sem: *mut Sem) -> c_int;
        /// Return a POSIX semaphore permit.
        pub fn sem_post(sem: *mut Sem) -> c_int;
    }
    #[cfg(target_os = "macos")]
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
}

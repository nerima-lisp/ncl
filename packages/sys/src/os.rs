//! Platform boundary declarations used by the runtime.

#[cfg(unix)]
pub mod declarations {
    use core::ffi::{c_char, c_int, c_void};
    #[repr(C)]
    pub struct Stat {
        _private: [u8; 0],
    }
    #[repr(C)]
    pub struct Dirent {
        _private: [u8; 0],
    }
    unsafe extern "C" {
        pub fn mmap(
            addr: *mut c_void,
            len: usize,
            prot: c_int,
            flags: c_int,
            fd: c_int,
            offset: isize,
        ) -> *mut c_void;
        pub fn munmap(addr: *mut c_void, len: usize) -> c_int;
        pub fn mprotect(addr: *mut c_void, len: usize, prot: c_int) -> c_int;
        pub fn pthread_create(
            thread: *mut usize,
            attr: *const c_void,
            start: Option<extern "C" fn(*mut c_void) -> *mut c_void>,
            arg: *mut c_void,
        ) -> c_int;
        pub fn pthread_join(thread: usize, value: *mut *mut c_void) -> c_int;
        pub fn pthread_self() -> usize;
        pub fn dlopen(path: *const c_char, flags: c_int) -> *mut c_void;
        pub fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
        pub fn dlerror() -> *const c_char;
        pub fn read(fd: c_int, buf: *mut c_void, count: usize) -> isize;
        pub fn write(fd: c_int, buf: *const c_void, count: usize) -> isize;
        pub fn open(path: *const c_char, flags: c_int, ...) -> c_int;
        pub fn close(fd: c_int) -> c_int;
        pub fn stat(path: *const c_char, output: *mut Stat) -> c_int;
        pub fn opendir(path: *const c_char) -> *mut Dirent;
        pub fn readdir(dir: *mut Dirent) -> *mut Dirent;
        pub fn closedir(dir: *mut Dirent) -> c_int;
        pub fn clock_gettime(clock: c_int, time: *mut c_void) -> c_int;
        pub fn signal(
            signum: c_int,
            handler: Option<extern "C" fn(c_int)>,
        ) -> Option<extern "C" fn(c_int)>;
    }
    #[cfg(target_os = "macos")]
    unsafe extern "C" {
        pub fn pthread_get_stackaddr_np(thread: usize) -> *mut c_void;
        pub fn pthread_get_stacksize_np(thread: usize) -> usize;
        pub fn pthread_jit_write_protect_np(enabled: c_int);
        pub fn sys_icache_invalidate(start: *const c_void, len: usize);
    }
    #[cfg(target_os = "linux")]
    unsafe extern "C" {
        pub fn pthread_getattr_np(thread: usize, attr: *mut c_void) -> c_int;
        pub fn pthread_attr_getstack(
            attr: *const c_void,
            stack: *mut *mut c_void,
            size: *mut usize,
        ) -> c_int;
    }
}

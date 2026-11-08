#[cfg(target_os = "linux")]
use core::ffi::c_long;
use core::ffi::{c_int, c_void};
use std::cell::RefCell;
use std::io;

mod alternate_stack;
mod disposition;
mod sigwait;
use alternate_stack::AltSignalStackOwner;

#[cfg(test)]
#[path = "../../tests/signal_control/private.rs"]
mod private_tests;

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

#[repr(C)]
struct SigAction {
    handler: usize,
    mask: SigSet,
    flags: c_int,
    #[cfg(target_os = "linux")]
    _restorer: *mut c_void,
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

fn signal_set(signals: &[c_int]) -> io::Result<SigSet> {
    disposition::create_signal_set(
        signals,
        || {
            // SAFETY: zero is a valid initial representation before sigemptyset fills it.
            unsafe { core::mem::zeroed() }
        },
        |set| {
            // SAFETY: set points to correctly sized storage for this target's sigset_t.
            unsafe { sigemptyset(&raw mut *set) }
        },
        |set, signal| {
            // SAFETY: set is initialized and signal is a valid target signal number.
            unsafe { sigaddset(&raw mut *set, signal) }
        },
        io::Error::last_os_error,
    )
}

fn wait_signal_set(signals: &[c_int]) -> io::Result<c_int> {
    let set = signal_set(signals)?;
    sigwait::wait_signal_set(&set, |set, signal| {
        // SAFETY: set is initialized and signal points to writable integer storage.
        unsafe { sigwait(&raw const *set, &raw mut *signal) }
    })
}

#[cfg(target_os = "linux")]
fn positive_sysconf(name: c_int, fallback: usize) -> usize {
    // SAFETY: sysconf accepts the Linux signal-stack configuration selectors.
    alternate_stack::positive_sysconf(unsafe { sysconf(name) }, fallback)
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
    alternate_stack::required_alt_signal_stack_size(
        minimum,
        recommended,
        page_size,
        ALT_SIGNAL_HANDLER_RESERVE,
    )
}

fn current_alt_signal_stack() -> io::Result<AltSignalStack> {
    // SAFETY: all fields in the supported `stack_t` layouts are valid when zeroed.
    let mut current: AltSignalStack = unsafe { core::mem::zeroed() };
    alternate_stack::query_alternate_signal_stack(
        // SAFETY: current points to writable stack_t storage; null requests a query only.
        unsafe { sigaltstack(core::ptr::null(), &raw mut current) },
        io::Error::last_os_error,
    )?;
    Ok(current)
}

impl Drop for AltSignalStackOwner {
    fn drop(&mut self) {
        let disabled_stack_size = self.memory.as_ref().map_or(0, |memory| memory.len());
        alternate_stack::release_alternate_signal_stack(
            self.memory.take(),
            || current_alt_signal_stack().map(|current| (current.sp.addr(), current.flags)),
            SS_DISABLE,
            SS_ONSTACK,
            || {
                let disabled = AltSignalStack {
                    sp: core::ptr::null_mut(),
                    // A zero-sized disable leaves the stack enabled on Darwin.
                    size: disabled_stack_size,
                    flags: SS_DISABLE,
                };
                // SAFETY: the current thread owns this alternate stack, and it is not active.
                alternate_stack::query_alternate_signal_stack(
                    unsafe { sigaltstack(&raw const disabled, core::ptr::null_mut()) },
                    io::Error::last_os_error,
                )
            },
            core::mem::forget,
        );
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
    alternate_stack::ensure_alternate_signal_stack(
        required,
        current.size,
        current.flags,
        SS_DISABLE,
        SS_ONSTACK,
        |required| {
            alternate_stack::map_alternate_signal_stack_tls(ALT_SIGNAL_STACK.try_with(|owner| {
                alternate_stack::register_alternate_signal_stack(
                    owner,
                    required,
                    |pointer, size| {
                        let stack = AltSignalStack {
                            sp: pointer.cast(),
                            size,
                            flags: 0,
                        };
                        // SAFETY: memory remains owned by thread-local storage for the stack's lifetime.
                        alternate_stack::query_alternate_signal_stack(
                            unsafe { sigaltstack(&raw const stack, core::ptr::null_mut()) },
                            io::Error::last_os_error,
                        )
                    },
                )
            }))
        },
    )
}

const extern "C" fn sigchld_wait_handler(_: c_int) {}

fn set_signal_disposition(signal: c_int, handler: usize) -> io::Result<()> {
    // SAFETY: zeroed storage is valid for sigaction's integer, pointer, and sigset fields.
    let mut action: SigAction = unsafe { core::mem::zeroed() };
    action.handler = handler;
    disposition::install_signal_disposition(
        &mut action,
        |action| {
            // SAFETY: action.mask has the target's sigset_t layout.
            unsafe { sigemptyset(&raw mut action.mask) }
        },
        |action| {
            // SAFETY: action has a valid handler and empty mask for this target's sigaction ABI.
            unsafe { sigaction(signal, &raw const *action, core::ptr::null_mut()) }
        },
        io::Error::last_os_error,
    )
}

fn block_signal_set(signals: &[c_int]) -> io::Result<()> {
    let set = signal_set(signals)?;
    // SAFETY: set is initialized and old_set is null because its prior value is unused.
    let result = unsafe { pthread_sigmask(SIG_BLOCK, &raw const set, core::ptr::null_mut()) };
    disposition::block_control_signal_mask(result)
}

impl crate::Thread {
    /// Ignore process-wide `SIGPIPE` so failed writes return `EPIPE` to normal code.
    ///
    /// # Errors
    /// Returns an OS error if the signal disposition cannot be set.
    pub fn ignore_sigpipe() -> io::Result<()> {
        set_signal_disposition(SIG_PIPE, SIG_IGN)
    }
    /// Install a no-op disposition so `SIGCHLD` can be consumed with `sigwait`.
    ///
    /// # Errors
    /// Returns an OS error if the signal disposition cannot be set.
    pub fn install_sigchld_wait_disposition() -> io::Result<()> {
        // check-added-lines: allow(as-cast) `sa_handler` is a function pointer in the POSIX ABI.
        set_signal_disposition(SIG_CHLD, sigchld_wait_handler as *const () as usize)
    }
    /// Block `SIGINT`, `SIGTERM`, and `SIGCHLD` on the calling OS thread.
    ///
    /// # Errors
    /// Returns an OS error if the signal mask cannot be updated.
    pub fn block_signal_control_set() -> io::Result<()> {
        block_signal_set(&[SIG_INT, SIG_TERM, SIG_CHLD])
    }
    /// Wait for one blocked control signal.
    /// # Errors
    /// Returns an OS error if `sigwait` fails.
    #[cfg(target_os = "linux")]
    pub fn wait_signal_control_set() -> io::Result<c_int> {
        wait_signal_set(&[SIG_INT, SIG_TERM, SIG_CHLD])
    }
    /// Wait for `SIGINT` or `SIGTERM`.
    /// # Errors
    /// Returns an OS error if `sigwait` fails.
    #[cfg(target_os = "macos")]
    pub fn wait_process_signal() -> io::Result<c_int> {
        wait_signal_set(&[SIG_INT, SIG_TERM])
    }
    /// Wait for `SIGCHLD`.
    /// # Errors
    /// Returns an OS error if `sigwait` fails.
    #[cfg(target_os = "macos")]
    pub fn wait_child_signal() -> io::Result<c_int> {
        wait_signal_set(&[SIG_CHLD])
    }

    /// Reuse or register a per-thread alternate signal stack of the contract size.
    ///
    /// # Errors
    /// Returns an OS error if stack sizing or registration fails.
    pub fn ensure_alt_signal_stack() -> io::Result<usize> {
        ensure_alt_signal_stack()
    }
}

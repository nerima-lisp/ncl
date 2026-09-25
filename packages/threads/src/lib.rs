//! The Lisp thread API over `ncl-sys`.
//!
//! This crate is the runtime side of the Common Lisp thread interface: thread
//! creation and lifecycle, mutexes (including recursive locks), read/write
//! locks, semaphores, wait queues, condition variables, timers, deadlines and
//! timeouts, and cooperative interrupts. It builds on the safe surface of
//! `ncl-object` (typed values, `ThreadContext`, roots) and `ncl-sys`
//! (safepoints, native transitions, heap registration) and contains no
//! low-level code of its own.
//!
//! Phase 1 registers every `NCL-THREADS` symbol assigned
//! to this crate by `conformance/ownership/symbols.tsv`, and implements the
//! runtime-side operations behind them. The `with-mutex`, `with-recursive-lock`,
//! `with-timeout`, `with-deadline`, `with-interrupts`, and related macro
//! expanders are owned by `ncl-lib-macros` (L19); this crate provides the
//! runtime entry points they lower to.
//!
//! # Phase 1 limitations
//!
//! - There is no Rust-to-Lisp call path yet, so a thread created by
//!   `thread-spawn` records its function object but does not invoke it; it
//!   registers its context, participates in cooperative safepoints, and waits
//!   until it is terminated.
//! - Symbol value cells are global defaults, so `*current-thread*` and
//!   thread-local symbol lookup do not model per-thread overrides.
//! - Spawning an OS thread uses `std::thread` because `ncl-sys` exposes
//!   `pthread_create` and `pthread_join` only as raw low-level declarations; a
//!   safe spawn/join wrapper is a needed `ncl-sys` addition.
//! - The process accessors operate on a process object model; spawning an OS
//!   process belongs to `ncl-ffi` and the runtime.

mod error;
mod mutex;
mod process;
mod register;
mod semaphore;
mod state;
mod symbols;
mod sync;
mod thread;
mod thread_object;
mod time;
mod timer;

pub use error::ThreadError;
pub use mutex::{
    get_mutex, holding_mutex_p, make_mutex, make_rwlock, mutex_name, mutex_owner, mutex_value,
    release_mutex, rwlock_rdlock, rwlock_unlock, rwlock_wrlock, with_mutex, with_recursive_lock,
};
pub use process::{
    Process, make_process, process_alive_p, process_close, process_core_dumped, process_error,
    process_exit_code, process_input, process_kill, process_output, process_p, process_pid,
    process_plist, process_pty, process_status, process_wait,
};
pub use register::register;
pub use semaphore::{
    clear_semaphore_notification, condition_broadcast, condition_notify, condition_wait,
    get_foreground, get_spinlock, make_semaphore, make_semaphore_notification, make_spinlock,
    make_waitqueue, release_foreground, release_spinlock, semaphore_count, semaphore_name,
    semaphore_notification_status, signal_semaphore, spinlock_held_p, try_semaphore,
    wait_on_semaphore, waitqueue_name,
};
pub use symbols::{SymbolKind, SymbolRow, rows};
pub use sync::MutexKind;
pub use thread::{
    FUNCTION_SLOT, ID_SLOT, MAIN_THREAD_ID, NAME_SLOT, STATE_FINISHED, STATE_RUNNING, STATE_SLOT,
    STATE_TERMINATED, ThreadBody, ThreadId, alive_p, all_ids, current_id, dispose_finished,
    idle_body, interrupt, join, os_tid, should_terminate, spawn, take_interrupt, terminate,
    thread_name, thread_yield,
};
pub use thread_object::{
    current_thread, finished_state, interrupt_thread, join_thread, list_all_threads, main_thread_p,
    make_main_thread_object, make_thread, object_id, terminate_thread, thread_alive_p,
    thread_error_thread, thread_name_of, thread_os_tid_of, thread_state_of,
};
pub use time::{
    call_with_timing, deadline_timeout_condition, decode_timeout, defer_deadline, enable_interrupt,
    get_time_of_day, pop_deadline, push_deadline, signal_deadline, with_deadline, with_interrupts,
    with_timeout, without_interrupts,
};
pub use timer::{
    TimerId, list_all_timers, make_timer, run_expired_timers, schedule_timer, timer_name,
    timer_scheduled_p, unschedule_timer,
};

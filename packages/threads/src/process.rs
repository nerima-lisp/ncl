//! Process objects and their accessors.
//!
//! `NCL-THREADS` assigns the `process-*` accessors to this crate, so Phase 1 models a
//! process as a heap instance with scalar slots. Spawning an OS process belongs
//! to `ncl-ffi` and the runtime; these functions read and update the object
//! model only, and [`process_kill`] marks the object dead without signalling a
//! real process.

use std::time::Duration;

use ncl_object::{Instance, Runtime, ThreadContext, Word, slot_ref};

use crate::ThreadError;
use crate::state::{make_object, read_slot, write_slot};

/// Slot index of the process identifier.
const PID_SLOT: usize = 0;
/// Slot index of the process status.
const STATUS_SLOT: usize = 1;
/// Slot index of the process property list.
const PLIST_SLOT: usize = 2;
/// Slot index of the process input stream.
const INPUT_SLOT: usize = 3;
/// Slot index of the process output stream.
const OUTPUT_SLOT: usize = 4;
/// Slot index of the process error stream.
const ERROR_SLOT: usize = 5;
/// Slot index of the core-dumped flag.
const CORE_SLOT: usize = 6;
/// Slot index of the pseudo-terminal flag.
const PTY_SLOT: usize = 7;
/// Slot index of the alive flag.
const ALIVE_SLOT: usize = 8;

/// Exit status recorded by [`process_kill`], matching the `SIGTERM` convention.
const KILLED_STATUS: i64 = -15;

/// An opaque handle to a process object.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Process(Word);

impl Process {
    /// Return the underlying word.
    #[must_use]
    pub const fn as_word(self) -> Word {
        self.0
    }

    /// Wrap a word as a process handle.
    #[must_use]
    pub const fn from_word(word: Word) -> Self {
        Self(word)
    }
}

impl From<Word> for Process {
    fn from(value: Word) -> Self {
        Self(value)
    }
}

impl From<Process> for Word {
    fn from(value: Process) -> Self {
        value.0
    }
}

/// Create a process object.
///
/// # Errors
/// Returns an object-layer error when the object cannot be built, and
/// [`ThreadError::NotAProcess`] when the identifier does not fit a fixnum.
pub fn make_process(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    pid: u64,
    status: Word,
) -> Result<Word, ThreadError> {
    let pid = i64::try_from(pid).map_err(|_| ThreadError::NotAProcess)?;
    let slots = [
        Word::fixnum(pid),
        status,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::TRUE,
    ];
    make_object(ctx, runtime, "PROCESS", &slots)
}

/// Return whether an object is a process.
///
/// # Errors
/// This operation does not fail; a non-process object reports NIL.
pub fn process_p(ctx: &ThreadContext, object: Word) -> Result<Word, ThreadError> {
    let is_process = slot_ref(ctx, Instance::from_word(object), ALIVE_SLOT).is_ok();
    Ok(if is_process { Word::TRUE } else { Word::NIL })
}

/// Return a process's identifier.
///
/// # Errors
/// Returns an object-layer error when the slot is malformed.
pub fn process_pid(ctx: &ThreadContext, process: Word) -> Result<Word, ThreadError> {
    read_slot(ctx, process, PID_SLOT)
}

/// Return a process's status.
///
/// # Errors
/// Returns an object-layer error when the slot is malformed.
pub fn process_status(ctx: &ThreadContext, process: Word) -> Result<Word, ThreadError> {
    read_slot(ctx, process, STATUS_SLOT)
}

/// Return a process's property list.
///
/// # Errors
/// Returns an object-layer error when the slot is malformed.
pub fn process_plist(ctx: &ThreadContext, process: Word) -> Result<Word, ThreadError> {
    read_slot(ctx, process, PLIST_SLOT)
}

/// Return a process's input stream.
///
/// # Errors
/// Returns an object-layer error when the slot is malformed.
pub fn process_input(ctx: &ThreadContext, process: Word) -> Result<Word, ThreadError> {
    read_slot(ctx, process, INPUT_SLOT)
}

/// Return a process's output stream.
///
/// # Errors
/// Returns an object-layer error when the slot is malformed.
pub fn process_output(ctx: &ThreadContext, process: Word) -> Result<Word, ThreadError> {
    read_slot(ctx, process, OUTPUT_SLOT)
}

/// Return a process's error stream.
///
/// # Errors
/// Returns an object-layer error when the slot is malformed.
pub fn process_error(ctx: &ThreadContext, process: Word) -> Result<Word, ThreadError> {
    read_slot(ctx, process, ERROR_SLOT)
}

/// Return whether a process dumped core.
///
/// # Errors
/// Returns an object-layer error when the slot is malformed.
pub fn process_core_dumped(ctx: &ThreadContext, process: Word) -> Result<Word, ThreadError> {
    read_slot(ctx, process, CORE_SLOT)
}

/// Return whether a process uses a pseudo-terminal.
///
/// # Errors
/// Returns an object-layer error when the slot is malformed.
pub fn process_pty(ctx: &ThreadContext, process: Word) -> Result<Word, ThreadError> {
    read_slot(ctx, process, PTY_SLOT)
}

/// Return whether a process is alive.
///
/// # Errors
/// Returns an object-layer error when the slot is malformed.
pub fn process_alive_p(ctx: &ThreadContext, process: Word) -> Result<Word, ThreadError> {
    let alive = read_slot(ctx, process, ALIVE_SLOT)?;
    Ok(if alive == Word::NIL {
        Word::NIL
    } else {
        Word::TRUE
    })
}

/// Return a process's exit status.
///
/// Phase 1 never spawns an OS process, so a live object has no status to wait
/// for: the recorded status is returned when the object is dead, and
/// [`ThreadError::Timeout`] is returned while it is alive.
///
/// # Errors
/// Returns [`ThreadError::Timeout`] while the object is alive.
pub fn process_wait(
    ctx: &mut ThreadContext,
    process: Word,
    _timeout: Option<Duration>,
) -> Result<Word, ThreadError> {
    if process_alive_p(ctx, process)? == Word::NIL {
        return read_slot(ctx, process, STATUS_SLOT);
    }
    Err(ThreadError::Timeout)
}

/// Mark a process object dead and record a killed status.
///
/// # Errors
/// Returns an object-layer error when the slots cannot be written.
pub fn process_kill(ctx: &mut ThreadContext, process: Word) -> Result<Word, ThreadError> {
    write_slot(ctx, process, ALIVE_SLOT, Word::NIL)?;
    write_slot(ctx, process, STATUS_SLOT, Word::fixnum(KILLED_STATUS))?;
    Ok(Word::TRUE)
}

/// Close a process object's streams.
///
/// # Errors
/// Returns an object-layer error when the slots cannot be written.
pub fn process_close(ctx: &mut ThreadContext, process: Word) -> Result<Word, ThreadError> {
    for slot in [INPUT_SLOT, OUTPUT_SLOT, ERROR_SLOT] {
        write_slot(ctx, process, slot, Word::NIL)?;
    }
    Ok(Word::TRUE)
}

/// Return a process's recorded exit code, or NIL while it is alive.
///
/// # Errors
/// Returns an object-layer error when the slot is malformed.
pub fn process_exit_code(ctx: &ThreadContext, process: Word) -> Result<Word, ThreadError> {
    if process_alive_p(ctx, process)? != Word::NIL {
        return Ok(Word::NIL);
    }
    read_slot(ctx, process, STATUS_SLOT)
}

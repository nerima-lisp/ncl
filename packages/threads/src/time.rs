//! Wall-clock time, timeout decoding, deadlines, and interrupt enablement.
//!
//! Deadlines are absolute monotonic nanoseconds held in a per-thread stack, as
//! the thread contract requires, so a wall-clock change cannot move a deadline.
//! Phase 1 has no asynchronous signal delivery, so [`with_deadline`] reports an
//! expiry after the protected form returns instead of interrupting it.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ncl_object::{ObjectError, Runtime, ThreadContext, Word, car, cdr, make_simple_vector};

use crate::ThreadError;
use crate::state::{intern_internal, lock};
use crate::thread::current_id;

/// Nanoseconds in one second.
const NANOS_PER_SECOND: u64 = 1_000_000_000;

static DEADLINES: OnceLock<Mutex<HashMap<u64, Vec<u64>>>> = OnceLock::new();
static INTERRUPTS: OnceLock<Mutex<HashMap<u64, bool>>> = OnceLock::new();
static MONOTONIC_BASE: OnceLock<Instant> = OnceLock::new();

fn deadlines() -> &'static Mutex<HashMap<u64, Vec<u64>>> {
    DEADLINES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn interrupts() -> &'static Mutex<HashMap<u64, bool>> {
    INTERRUPTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn monotonic_nanos() -> u64 {
    MONOTONIC_BASE
        .get_or_init(Instant::now)
        .elapsed()
        .as_nanos()
        .try_into()
        .unwrap_or(u64::MAX)
}

/// Return the current wall-clock time as seconds since the Unix epoch.
#[must_use]
pub fn get_time_of_day() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn seconds_to_duration(seconds: Word) -> Result<Duration, ThreadError> {
    let value = seconds
        .as_fixnum()
        .ok_or(ThreadError::Object(ObjectError::TypeError))?;
    if value < 0 {
        return Err(ThreadError::Object(ObjectError::TypeError));
    }
    Ok(Duration::from_secs(u64::try_from(value).map_err(|_| {
        ThreadError::Object(ObjectError::TypeError)
    })?))
}

/// Decode a Lisp timeout designator into a duration.
///
/// Accepts NIL (no timeout), a non-negative fixnum of seconds, and a cons
/// `(seconds . nanoseconds)`.
///
/// # Errors
/// Returns a type error for any other designator, and a type error when the
/// nanosecond part is out of range.
pub fn decode_timeout(
    ctx: &mut ThreadContext,
    timeout: Word,
) -> Result<Option<Duration>, ThreadError> {
    if timeout == Word::NIL {
        return Ok(None);
    }
    if let Some(seconds) = timeout.as_fixnum() {
        return Ok(Some(seconds_to_duration(Word::fixnum(seconds))?));
    }
    if timeout.is_cons() {
        let seconds = car(ctx, timeout)?;
        let nanos = cdr(ctx, timeout)?;
        let seconds = seconds_to_duration(seconds)?;
        let nanos = nanos
            .as_fixnum()
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| u64::from(*value) < NANOS_PER_SECOND)
            .ok_or(ThreadError::Object(ObjectError::TypeError))?;
        return Ok(Some(seconds + Duration::from_nanos(u64::from(nanos))));
    }
    Err(ThreadError::Object(ObjectError::TypeError))
}

fn absolute_deadline(seconds: Word) -> Result<u64, ThreadError> {
    let duration = seconds_to_duration(seconds)?;
    let nanos: u64 = duration.as_nanos().try_into().unwrap_or(u64::MAX);
    Ok(monotonic_nanos().saturating_add(nanos))
}

fn push_absolute(thread: u64, deadline: u64) -> Option<u64> {
    let mut table = lock(deadlines());
    let previous = {
        let stack = table.entry(thread).or_default();
        let previous = stack.last().copied();
        stack.push(deadline);
        previous
    };
    drop(table);
    previous
}

fn pop_absolute(thread: u64) -> Option<u64> {
    lock(deadlines()).get_mut(&thread).and_then(Vec::pop)
}

fn top_absolute(thread: u64) -> Option<u64> {
    lock(deadlines())
        .get(&thread)
        .and_then(|stack| stack.last().copied())
}

fn deadline_word(nanos: Option<u64>) -> Word {
    nanos
        .and_then(|value| i64::try_from(value).ok())
        .map_or(Word::NIL, Word::fixnum)
}

/// Push an absolute deadline `seconds` from now and return the previous one.
///
/// # Errors
/// Returns a type error when `seconds` is not a non-negative fixnum.
pub fn push_deadline(_ctx: &mut ThreadContext, seconds: Word) -> Result<Word, ThreadError> {
    let deadline = absolute_deadline(seconds)?;
    Ok(deadline_word(push_absolute(current_id().get(), deadline)))
}

/// Pop the innermost deadline and return it.
///
/// # Errors
/// This operation does not fail.
pub fn pop_deadline(_ctx: &mut ThreadContext) -> Result<Word, ThreadError> {
    Ok(deadline_word(pop_absolute(current_id().get())))
}

/// Return the innermost deadline without removing it.
///
/// # Errors
/// This operation does not fail.
pub fn signal_deadline(_ctx: &ThreadContext) -> Result<Word, ThreadError> {
    Ok(deadline_word(top_absolute(current_id().get())))
}

/// Move the innermost deadline to `seconds` from now and return it.
///
/// # Errors
/// Returns a type error when `seconds` is not a non-negative fixnum.
pub fn defer_deadline(_ctx: &mut ThreadContext, seconds: Word) -> Result<Word, ThreadError> {
    let deadline = absolute_deadline(seconds)?;
    let thread = current_id().get();
    let mut table = lock(deadlines());
    if let Some(stack) = table.get_mut(&thread)
        && let Some(slot) = stack.last_mut()
    {
        *slot = deadline;
    }
    drop(table);
    Ok(Word::fixnum(i64::try_from(deadline).map_err(|_| {
        ThreadError::Object(ObjectError::TypeError)
    })?))
}

/// Set the calling thread's interrupt enablement and return the previous value.
///
/// # Errors
/// This operation does not fail.
pub fn enable_interrupt(_ctx: &mut ThreadContext, enabled: bool) -> Result<Word, ThreadError> {
    let previous = lock(interrupts()).insert(current_id().get(), enabled);
    Ok(if previous.unwrap_or(true) {
        Word::TRUE
    } else {
        Word::NIL
    })
}

fn interrupt_enabled(thread: u64) -> bool {
    lock(interrupts()).get(&thread).copied().unwrap_or(true)
}

/// Run `f` with an absolute deadline `seconds` from now.
///
/// # Errors
/// Returns `f`'s error, or [`ThreadError::Timeout`] when the deadline passed by
/// the time `f` returned.
pub fn with_deadline<T>(
    ctx: &mut ThreadContext,
    seconds: Word,
    f: impl FnOnce(&mut ThreadContext) -> Result<T, ThreadError>,
) -> Result<T, ThreadError> {
    let deadline = absolute_deadline(seconds)?;
    let _ = push_absolute(current_id().get(), deadline);
    let result = f(ctx);
    let _ = pop_absolute(current_id().get());
    match result {
        Ok(_) if monotonic_nanos() >= deadline => Err(ThreadError::Timeout),
        other => other,
    }
}

/// Run `f` with a timeout, binding `NCL-THREADS:*TIMEOUT-EXIT*` while it runs.
///
/// # Errors
/// Returns `f`'s error, or [`ThreadError::Timeout`] when the timeout passed by
/// the time `f` returned.
pub fn with_timeout<T>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    seconds: Word,
    f: impl FnOnce(&mut ThreadContext) -> Result<T, ThreadError>,
) -> Result<T, ThreadError> {
    let symbol = intern_internal(ctx, runtime, "NCL-THREADS", "*TIMEOUT-EXIT*")?;
    let previous = ncl_object::symbol_value(ctx, symbol)?;
    ncl_object::set_symbol_value(ctx, symbol, seconds)?;
    let result = with_deadline(ctx, seconds, f);
    ncl_object::set_symbol_value(ctx, symbol, previous)?;
    result
}

/// Run `f` with interrupts enabled.
///
/// # Errors
/// Returns `f`'s error.
pub fn with_interrupts<T>(
    ctx: &mut ThreadContext,
    f: impl FnOnce(&mut ThreadContext) -> Result<T, ThreadError>,
) -> Result<T, ThreadError> {
    let thread = current_id().get();
    let previous = interrupt_enabled(thread);
    let _ = enable_interrupt(ctx, true);
    let result = f(ctx);
    let _ = enable_interrupt(ctx, previous);
    result
}

/// Run `f` with interrupts disabled.
///
/// # Errors
/// Returns `f`'s error.
pub fn without_interrupts<T>(
    ctx: &mut ThreadContext,
    f: impl FnOnce(&mut ThreadContext) -> Result<T, ThreadError>,
) -> Result<T, ThreadError> {
    let thread = current_id().get();
    let previous = interrupt_enabled(thread);
    let _ = enable_interrupt(ctx, false);
    let result = f(ctx);
    let _ = enable_interrupt(ctx, previous);
    result
}

/// Build an `NCL-THREADS:TIMEOUT-DEADLINE` condition instance.
///
/// The condition class is owned by `ncl-conditions`, so this crate does not
/// signal it directly: the `with-deadline` and `with-timeout` expanders in
/// `ncl-lib-macros` (L19) lower to this entry point and to
/// [`ThreadError::Timeout`].
///
/// # Errors
/// Returns [`ThreadError::MissingClass`] when `ncl-conditions` has not registered
/// the deadline condition class, and a condition error when construction fails.
pub fn deadline_timeout_condition(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) -> Result<Word, ThreadError> {
    let class = ncl_conditions::condition_class(ctx, runtime, "DEADLINE-TIMEOUT")
        .ok_or(ThreadError::MissingClass)?;
    Ok(ncl_conditions::make_condition(ctx, runtime, class, &[])?)
}

/// Run `f` and return `[elapsed-milliseconds, result]`.
///
/// # Errors
/// Returns `f`'s error.
pub fn call_with_timing(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    f: impl FnOnce(&mut ThreadContext) -> Result<Word, ThreadError>,
) -> Result<Word, ThreadError> {
    let started = Instant::now();
    let value = f(ctx)?;
    let elapsed = u64::try_from(started.elapsed().as_millis())
        .map_err(|_| ThreadError::Object(ObjectError::TypeError))?;
    let millis = i64::try_from(elapsed).map_err(|_| ThreadError::Object(ObjectError::TypeError))?;
    Ok(make_simple_vector(
        ctx,
        runtime,
        &[Word::fixnum(millis), value],
    )?)
}

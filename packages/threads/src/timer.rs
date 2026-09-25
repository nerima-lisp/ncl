//! Timer objects and the timer registry.
//!
//! A timer is a heap instance carrying a name and a scalar handle; the absolute
//! deadline lives in a process-wide table keyed by that handle. Firing a timer
//! requires the Rust-to-Lisp call path, so Phase 1 exposes
//! [`run_expired_timers`] as the deterministic driver a scheduler will use.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use ncl_object::{Runtime, ThreadContext, Word, make_cons, set_symbol_value, symbol_value};

use crate::ThreadError;
use crate::state::{intern_internal, lock, make_named_object, read_handle, read_slot, with_root};

/// A unique timer identifier.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TimerId(u64);

impl TimerId {
    /// Return the raw identifier.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug)]
struct TimerRecord {
    deadline: Option<u64>,
}

static TIMERS: OnceLock<Mutex<HashMap<u64, TimerRecord>>> = OnceLock::new();

fn timers() -> &'static Mutex<HashMap<u64, TimerRecord>> {
    TIMERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn timer_handle(ctx: &ThreadContext, timer: Word) -> Result<u64, ThreadError> {
    let handle = read_handle(ctx, timer, 1).map_err(|_| ThreadError::NotATimer)?;
    if lock(timers()).contains_key(&handle) {
        Ok(handle)
    } else {
        Err(ThreadError::NotATimer)
    }
}

/// Create an unscheduled timer.
///
/// # Errors
/// Returns an object-layer error when the object cannot be built.
///
/// # Panics
/// Panics if a root token cannot be removed in stack order.
pub fn make_timer(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<Word, ThreadError> {
    let handle = {
        let mut table = lock(timers());
        let handle = table.len() as u64 + 1;
        table.insert(handle, TimerRecord { deadline: None });
        handle
    };
    let mut timer = make_named_object(ctx, runtime, "TIMER", name, handle, &[])?;
    with_root(ctx, &mut timer, |ctx, timer| {
        push_all_timers(ctx, runtime, *timer)
    })?;
    Ok(timer)
}

fn push_all_timers(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    timer: Word,
) -> Result<(), ThreadError> {
    let symbol = intern_internal(ctx, runtime, "NCL-THREADS", "*TIMERS*")?;
    let mut timer = timer;
    with_root(ctx, &mut timer, |ctx, timer| {
        let mut list = symbol_value(ctx, symbol)?;
        if list == Word::UNBOUND {
            list = Word::NIL;
        }
        with_root(ctx, &mut list, |ctx, list| {
            let cons = make_cons(ctx, runtime, *timer, *list)?;
            set_symbol_value(ctx, symbol, cons)?;
            Ok(())
        })
    })
}

/// Schedule a timer for an absolute monotonic deadline in nanoseconds.
///
/// # Errors
/// Returns [`ThreadError::NotATimer`] for an unknown timer.
pub fn schedule_timer(
    ctx: &mut ThreadContext,
    timer: Word,
    deadline_nanos: u64,
) -> Result<Word, ThreadError> {
    let handle = timer_handle(ctx, timer)?;
    if let Some(record) = lock(timers()).get_mut(&handle) {
        record.deadline = Some(deadline_nanos);
    }
    Ok(Word::TRUE)
}

/// Cancel a timer's schedule.
///
/// # Errors
/// Returns [`ThreadError::NotATimer`] for an unknown timer.
pub fn unschedule_timer(ctx: &mut ThreadContext, timer: Word) -> Result<Word, ThreadError> {
    let handle = timer_handle(ctx, timer)?;
    if let Some(record) = lock(timers()).get_mut(&handle) {
        record.deadline = None;
    }
    Ok(Word::TRUE)
}

/// Return a timer's name.
///
/// # Errors
/// Returns an object-layer error when the name slot is malformed.
pub fn timer_name(ctx: &ThreadContext, timer: Word) -> Result<Word, ThreadError> {
    read_slot(ctx, timer, 0)
}

/// Return whether a timer is currently scheduled.
///
/// # Errors
/// Returns [`ThreadError::NotATimer`] for an unknown timer.
pub fn timer_scheduled_p(ctx: &ThreadContext, timer: Word) -> Result<Word, ThreadError> {
    let handle = timer_handle(ctx, timer)?;
    let scheduled = lock(timers())
        .get(&handle)
        .is_some_and(|record| record.deadline.is_some());
    Ok(if scheduled { Word::TRUE } else { Word::NIL })
}

/// Return the `NCL-THREADS::*TIMERS*` list of timer objects.
///
/// # Errors
/// Returns an object-layer error when the internal symbol cannot be interned.
pub fn list_all_timers(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Word, ThreadError> {
    let symbol = intern_internal(ctx, runtime, "NCL-THREADS", "*TIMERS*")?;
    let list = symbol_value(ctx, symbol)?;
    Ok(if list == Word::UNBOUND {
        Word::NIL
    } else {
        list
    })
}

/// Unschedule and return every timer whose deadline is at or before `now_nanos`.
#[must_use]
pub fn run_expired_timers(now_nanos: u64) -> Vec<TimerId> {
    let mut expired: Vec<u64> = {
        let mut table = lock(timers());
        table
            .iter_mut()
            .filter_map(|(handle, record)| {
                let deadline = record.deadline?;
                if deadline > now_nanos {
                    return None;
                }
                record.deadline = None;
                Some(*handle)
            })
            .collect()
    };
    expired.sort_unstable();
    expired.into_iter().map(TimerId).collect()
}

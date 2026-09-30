//! Conditions, handlers, restarts, catch, and cleanup.
//!
//! This crate owns the runtime side of the Common Lisp condition system: the
//! standard condition hierarchy, the handler / restart / cleanup / catch
//! record chains (three current pointers in `ThreadContext`, per the native
//! backend contract), and the signalling entry points `signal`, `error`,
//! `warn`, and `cerror`. The `handler-bind`, `handler-case`, and
//! `restart-case` macro expanders live in `ncl-lib-macros` (L19); this crate
//! provides the record-chain primitives they lower to.
//!
//! Phase 1 records are heap-allocated simple vectors chained by a `previous`
//! slot. A condition class is a minimal descriptor vector, and a condition
//! instance is a CLOS-style instance whose class word is that descriptor. The
//! machine-side unwind transfer is the L13/L14 unwinder work.

#![forbid(unsafe_code)]

mod builtin_slot_specs;
mod class;
mod conversion;
mod define_condition;
mod error;
mod handler;
mod hierarchy;
pub(crate) mod records;
mod register;
mod report;
mod restart;
mod restart_builtins;
pub(crate) mod slots;
mod symbols;

pub use class::{
    ConditionClass, ConditionIdentifier, ConditionRecord, ConditionSlotValue, condition_class,
    condition_class_name, condition_class_of, make_condition, make_condition_record,
    make_typed_condition,
};
pub use conversion::condition_from_lisp_error;
pub use error::ConditionError;
pub use handler::{
    HandlerChain, cerror, error, pop_handler, push_handler, signal, signal_matched, warn,
};
pub use register::register;
pub use report::condition_report;
pub use restart::{
    CleanupRecord, RestartRecord, compute_restarts, find_restart, invoke_restart,
    invoke_restart_by_name, pop_cleanup, pop_restart, push_cleanup, push_restart, restart_name,
    unwind,
};

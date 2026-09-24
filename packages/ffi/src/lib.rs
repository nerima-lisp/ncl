//! Foreign declarations and calls.
//!
//! `ncl-ffi` owns the SBCL `SB-ALIEN` / `SB-SYS` surface: alien type
//! descriptors with their size and alignment, `alien-funcall` argument
//! marshalling, system area pointers, dynamic loading, and the 106 symbols the
//! ownership table assigns to this crate.
//!
//! Dynamic loading and foreign calls reach the OS only through `ncl-sys`, and
//! this crate stays within Rust's safe subset, so every raw pointer operation
//! remains behind `ncl-sys`. `ncl-sys` currently exposes only raw `extern "C"`
//! declarations for `dlopen` / `dlsym` / `dlerror` and for reading and writing
//! an arbitrary address, so the operations that need them return
//! [`FfiError::MissingSysPrimitive`] naming the required signature. See
//! [`sys_requirements`] and the crate README for the exact requests.

#![forbid(unsafe_code)]

mod alien;
mod condition;
mod dynamic;
mod error;
mod funcall;
mod memory;
mod register;
mod roots;
mod sap;
mod symbols;
pub mod sys_requirements;

pub use alien::{
    AlienEnum, AlienRecord, AlienRoutine, AlienType, align_of, marshal_argument, offset_of,
    parse_type_name, parse_type_specifier, record_size, size_of, union_size, unmarshal_result,
};
pub use condition::signal_ffi_error;
pub use dynamic::{
    SharedObject, dlerror_message, dlopen_or_lose, extern_alien_name,
    find_dynamic_foreign_symbol_address, find_foreign_symbol_address, foreign_symbol_address,
    foreign_symbol_dataref_sap, foreign_symbol_sap, load_shared_object, unload_shared_object,
};
pub use error::FfiError;
pub use funcall::{alien_funcall, alien_routine, alien_sap, alien_size, cast, null_alien};
pub use memory::{
    allocate_system_memory, deallocate_system_memory, memmove, sap_ref, sap_set,
    with_rooted_objects,
};
pub use register::register;
pub use sap::{SystemAreaPointer, sap_eq, sap_ge, sap_gt, sap_le, sap_lt};

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
mod error;
mod sap;
pub mod sys_requirements;

pub use alien::{
    AlienEnum, AlienRecord, AlienRoutine, AlienType, align_of, marshal_argument, offset_of,
    parse_type_name, parse_type_specifier, record_size, size_of, union_size, unmarshal_result,
};
pub use error::FfiError;
pub use sap::{SystemAreaPointer, sap_eq, sap_ge, sap_gt, sap_le, sap_lt};

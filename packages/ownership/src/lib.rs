//! Test-support helpers that check per-crate symbol coverage against the
//! ownership table.
//!
//! `ncl-ownership` is not part of the runtime. It verifies that a crate
//! registered every symbol in the table supplied by its coverage test:
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let runtime = ncl_object::Runtime::new()?;
//! let mut ctx = ncl_object::ThreadContext::new();
//! ctx.register(&runtime)?;
//! // `ncl_<crate>::register(&Runtime)` is implemented by each lane.
//! const TABLE: &str = include_str!("../../types/ownership.tsv");
//! ncl_ownership::assert_crate_coverage_from_table(
//!     &runtime, &mut ctx, TABLE, "ncl-types",
//! )?;
//! # Ok(())
//! # }
//! ```
//!
//! Each failure prints as `package::symbol (kind): reason`, one line per
//! missing symbol, and a crate with no Phase 1 rows fails with
//! [`OwnershipError::NoRows`] instead of passing vacuously.
//!
//! # Checks performed
//!
//! | kind | check |
//! | --- | --- |
//! | package | [`Runtime::find_package`](ncl_object::Runtime::find_package) finds it |
//! | symbol | [`Package::find_symbol`](ncl_object::Package::find_symbol) interns it |
//! | `function` | [`Runtime::function`](ncl_object::Runtime::function) has a registered function |
//! | `class`, `condition` | [`Runtime::class`](ncl_object::Runtime::class) has a registered class |
//! | `macro` | [`symbol_is_macro`](ncl_object::symbol_is_macro) is set |
//! | `variable` | [`symbol_is_special`](ncl_object::symbol_is_special) is set |
//! | `constant` | [`symbol_is_constant`](ncl_object::symbol_is_constant) is set |
//! | `type`, `special-operator`, `other` | interned only |
//!
//! # Known gaps
//!
//! Functions must be registered with
//! [`Runtime::define_function`](ncl_object::Runtime::define_function) and
//! classes with [`Runtime::define_class`](ncl_object::Runtime::define_class)
//! for the gate to see them.

mod coverage;
mod symbols;
mod table;

pub use coverage::{
    Missing, OwnershipError, assert_crate_coverage_from_table,
    assert_crate_function_bindings_from_table,
};
pub use symbols::{SymbolKind, SymbolRow};
pub use table::{Kind, Row, rows_for_crate_from_str, rows_from_str};

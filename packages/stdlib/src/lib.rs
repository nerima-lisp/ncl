//! Standard-library registration ordering.

use ncl_object::{ObjectError, Runtime, ThreadContext};

/// Frozen order for standard-library and extension registration.
pub const REGISTRATION_ORDER: &[&str] = &[
    "ncl-types",
    "ncl-reader",
    "ncl-printer",
    "ncl-conditions",
    "ncl-clos",
    "ncl-lib-numbers",
    "ncl-lib-sequences",
    "ncl-lib-strings",
    "ncl-lib-hash-arrays",
    "ncl-lib-streams",
    "ncl-lib-pathnames",
    "ncl-lib-packages",
    "ncl-lib-format",
    "ncl-lib-macros",
    "ncl-threads",
    "ncl-ffi",
    "ncl-image",
    "ncl-os",
    "ncl-uiop",
    "ncl-asdf",
    "ncl-profiler",
    "ncl-coverage",
    "ncl-debug",
    "ncl-disasm",
];

/// Register every standard-library crate that currently exposes a registration
/// entry point, in [`REGISTRATION_ORDER`].
///
/// Crates without a registration function are intentionally skipped until
/// their implementation lane lands.
///
/// # Errors
///
/// Returns the first [`ObjectError`] reported by a crate registration.
pub fn register_all(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    ncl_types::register(runtime)?;
    ncl_reader::register(runtime)?;
    ncl_printer::register(ctx, runtime)?;
    ncl_conditions::register(runtime)?;
    ncl_clos::register(runtime)?;
    ncl_lib_macros::register(runtime)?;
    ncl_threads::register(runtime)?;
    ncl_ffi::register(runtime)?;
    ncl_image::register(runtime)?;
    Ok(())
}

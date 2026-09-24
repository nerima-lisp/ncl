//! Dynamic loading of shared objects.
//!
//! Every operation here is gated on a safe `ncl-sys` wrapper for `dlopen`,
//! `dlsym`, and `dlerror`. `ncl-sys` currently exposes only the raw `extern "C"`
//! declarations in `ncl_sys::os::declarations`, which this crate cannot call, so
//! each function returns [`FfiError::MissingSysPrimitive`] naming the required
//! signature.

use crate::FfiError;
use crate::sap::SystemAreaPointer;
use crate::sys_requirements::{
    DLCLOSE_SHARED_OBJECT, DLERROR_MESSAGE, DLOPEN_SHARED_OBJECT, DLSYM_FOREIGN_SYMBOL,
};

/// An opaque, owned handle to a loaded shared object.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SharedObject(usize);

impl SharedObject {
    /// Wrap a loader handle.
    #[must_use]
    pub const fn new(handle: usize) -> Self {
        Self(handle)
    }

    /// The wrapped loader handle.
    #[must_use]
    pub const fn handle(self) -> usize {
        self.0
    }
}

/// Load a shared object, returning an owned handle, like `load-shared-object`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// `dlopen` wrapper.
pub const fn load_shared_object(_path: &str, _lazy: bool) -> Result<SharedObject, FfiError> {
    Err(FfiError::MissingSysPrimitive(DLOPEN_SHARED_OBJECT))
}

/// Close a shared object handle, like `unload-shared-object`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// `dlclose` wrapper.
pub const fn unload_shared_object(_object: SharedObject) -> Result<(), FfiError> {
    Err(FfiError::MissingSysPrimitive(DLCLOSE_SHARED_OBJECT))
}

/// Load a shared object or signal an error, like `dlopen-or-lose`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// `dlopen` wrapper.
pub const fn dlopen_or_lose(_path: &str) -> Result<SharedObject, FfiError> {
    Err(FfiError::MissingSysPrimitive(DLOPEN_SHARED_OBJECT))
}

/// Resolve `name` in `object`, like `find-dynamic-foreign-symbol-address`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// `dlsym` wrapper.
pub const fn find_dynamic_foreign_symbol_address(
    _object: SharedObject,
    _name: &str,
) -> Result<SystemAreaPointer, FfiError> {
    Err(FfiError::MissingSysPrimitive(DLSYM_FOREIGN_SYMBOL))
}

/// Resolve `name` in the global namespace, like `find-foreign-symbol-address`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// `dlsym` wrapper.
pub const fn find_foreign_symbol_address(_name: &str) -> Result<SystemAreaPointer, FfiError> {
    Err(FfiError::MissingSysPrimitive(DLSYM_FOREIGN_SYMBOL))
}

/// Resolve `name` or signal an error, like `foreign-symbol-address`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// `dlsym` wrapper.
pub const fn foreign_symbol_address(_name: &str) -> Result<SystemAreaPointer, FfiError> {
    Err(FfiError::MissingSysPrimitive(DLSYM_FOREIGN_SYMBOL))
}

/// Resolve `name` to a SAP, like `foreign-symbol-sap`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// `dlsym` wrapper.
pub const fn foreign_symbol_sap(_name: &str) -> Result<SystemAreaPointer, FfiError> {
    Err(FfiError::MissingSysPrimitive(DLSYM_FOREIGN_SYMBOL))
}

/// Resolve `name` to the SAP of its variable storage, like
/// `foreign-symbol-dataref-sap`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// `dlsym` wrapper.
pub const fn foreign_symbol_dataref_sap(_name: &str) -> Result<SystemAreaPointer, FfiError> {
    Err(FfiError::MissingSysPrimitive(DLSYM_FOREIGN_SYMBOL))
}

/// Return the most recent dynamic-loader error message, like `dlerror`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// `dlerror` wrapper.
pub const fn dlerror_message() -> Result<Option<String>, FfiError> {
    Err(FfiError::MissingSysPrimitive(DLERROR_MESSAGE))
}

/// The C symbol name for a Lisp alien name, like `extern-alien-name`.
///
/// Phase 1 uses the Lisp name verbatim; the linkage-table mapping that would
/// rewrite it is the `update-alien-linkage-table` work, which needs `dlsym`.
#[must_use]
pub fn extern_alien_name(name: &str) -> String {
    name.to_owned()
}

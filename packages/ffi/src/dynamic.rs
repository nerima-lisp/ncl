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
///
/// Handles cannot be copied or cloned, and consuming one is required to unload
/// it.
#[derive(Debug, Eq, PartialEq, Hash)]
pub struct SharedObject(usize);

/// Whether a shared object is resolved lazily or immediately by the loader.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum LoaderMode {
    /// Resolve symbols as they are used.
    Lazy,
    /// Resolve symbols while loading the object.
    Now,
}

impl From<bool> for LoaderMode {
    fn from(lazy: bool) -> Self {
        if lazy { Self::Lazy } else { Self::Now }
    }
}

/// An owned path passed to the dynamic loader.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SharedObjectPath(String);

impl SharedObjectPath {
    #[must_use]
    /// Construct a path value object.
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    #[must_use]
    /// Borrow the path text at the dynamic-loader boundary.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for SharedObjectPath {
    fn from(path: &str) -> Self {
        Self::new(path)
    }
}

/// An owned name passed to the dynamic symbol loader.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ForeignSymbolName(String);

impl ForeignSymbolName {
    #[must_use]
    /// Construct a foreign symbol name value object.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    #[must_use]
    /// Borrow the symbol name at the dynamic-loader boundary.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ForeignSymbolName {
    fn from(name: &str) -> Self {
        Self::new(name)
    }
}

/// Load a shared object, returning an owned handle, like `load-shared-object`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// `dlopen` wrapper.
pub fn load_shared_object(
    path: impl Into<SharedObjectPath>,
    mode: impl Into<LoaderMode>,
) -> Result<SharedObject, FfiError> {
    let _path = path.into();
    let _mode = mode.into();
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
pub fn dlopen_or_lose(path: impl Into<SharedObjectPath>) -> Result<SharedObject, FfiError> {
    let _path = path.into();
    Err(FfiError::MissingSysPrimitive(DLOPEN_SHARED_OBJECT))
}

/// Resolve `name` in `object`, like `find-dynamic-foreign-symbol-address`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// `dlsym` wrapper.
pub fn find_dynamic_foreign_symbol_address(
    _object: SharedObject,
    name: impl Into<ForeignSymbolName>,
) -> Result<SystemAreaPointer, FfiError> {
    let _name = name.into();
    Err(FfiError::MissingSysPrimitive(DLSYM_FOREIGN_SYMBOL))
}

/// Resolve `name` in the global namespace, like `find-foreign-symbol-address`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// `dlsym` wrapper.
pub fn find_foreign_symbol_address(
    name: impl Into<ForeignSymbolName>,
) -> Result<SystemAreaPointer, FfiError> {
    let _name = name.into();
    Err(FfiError::MissingSysPrimitive(DLSYM_FOREIGN_SYMBOL))
}

/// Resolve `name` or signal an error, like `foreign-symbol-address`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// `dlsym` wrapper.
pub fn foreign_symbol_address(
    name: impl Into<ForeignSymbolName>,
) -> Result<SystemAreaPointer, FfiError> {
    let _name = name.into();
    Err(FfiError::MissingSysPrimitive(DLSYM_FOREIGN_SYMBOL))
}

/// Resolve `name` to a SAP, like `foreign-symbol-sap`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// `dlsym` wrapper.
pub fn foreign_symbol_sap(
    name: impl Into<ForeignSymbolName>,
) -> Result<SystemAreaPointer, FfiError> {
    let _name = name.into();
    Err(FfiError::MissingSysPrimitive(DLSYM_FOREIGN_SYMBOL))
}

/// Resolve `name` to the SAP of its variable storage, like
/// `foreign-symbol-dataref-sap`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// `dlsym` wrapper.
pub fn foreign_symbol_dataref_sap(
    name: impl Into<ForeignSymbolName>,
) -> Result<SystemAreaPointer, FfiError> {
    let _name = name.into();
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

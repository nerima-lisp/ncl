//! Dynamic loading of shared objects.
//!
//! Every operation here is gated on a safe `ncl-sys` wrapper for `dlopen`,
//! `dlsym`, and `dlerror`.

#![allow(clippy::as_conversions, clippy::needless_pass_by_value)]

use crate::FfiError;
use crate::sap::SystemAreaPointer;

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
    let path = path.into();
    let mode = mode.into();
    ncl_sys::ffi::dlopen_shared_object(path.as_str(), matches!(mode, LoaderMode::Lazy))
        .map(|handle| SharedObject(handle.as_raw() as usize))
        .map_err(|error| FfiError::DynamicLoader(error.to_string()))
}

/// Close a shared object handle, like `unload-shared-object`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// `dlclose` wrapper.
pub fn unload_shared_object(object: SharedObject) -> Result<(), FfiError> {
    ncl_sys::ffi::dlclose_shared_object(ncl_sys::ffi::SharedObject::from_raw(
        object.0 as *mut core::ffi::c_void,
    ))
    .map_err(|error| FfiError::DynamicLoader(error.to_string()))
}

/// Load a shared object or signal an error, like `dlopen-or-lose`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// `dlopen` wrapper.
pub fn dlopen_or_lose(path: impl Into<SharedObjectPath>) -> Result<SharedObject, FfiError> {
    load_shared_object(path, LoaderMode::Now)
}

/// Resolve `name` in `object`, like `find-dynamic-foreign-symbol-address`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// `dlsym` wrapper.
pub fn find_dynamic_foreign_symbol_address(
    object: &SharedObject,
    name: impl Into<ForeignSymbolName>,
) -> Result<SystemAreaPointer, FfiError> {
    let name = name.into();
    ncl_sys::ffi::dlsym_foreign_symbol(
        Some(&ncl_sys::ffi::SharedObject::from_raw(
            object.0 as *mut core::ffi::c_void,
        )),
        name.as_str(),
    )
    .map(SystemAreaPointer::new)
    .map_err(|error| FfiError::DynamicLoader(error.to_string()))
}

/// Resolve `name` in the global namespace, like `find-foreign-symbol-address`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// `dlsym` wrapper.
pub fn find_foreign_symbol_address(
    name: impl Into<ForeignSymbolName>,
) -> Result<SystemAreaPointer, FfiError> {
    let name = name.into();
    ncl_sys::ffi::dlsym_foreign_symbol(None, name.as_str())
        .map(SystemAreaPointer::new)
        .map_err(|error| FfiError::DynamicLoader(error.to_string()))
}

/// Resolve `name` or signal an error, like `foreign-symbol-address`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// `dlsym` wrapper.
pub fn foreign_symbol_address(
    name: impl Into<ForeignSymbolName>,
) -> Result<SystemAreaPointer, FfiError> {
    find_foreign_symbol_address(name)
}

/// Resolve `name` to a SAP, like `foreign-symbol-sap`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// `dlsym` wrapper.
pub fn foreign_symbol_sap(
    name: impl Into<ForeignSymbolName>,
) -> Result<SystemAreaPointer, FfiError> {
    find_foreign_symbol_address(name)
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
    find_foreign_symbol_address(name)
}

/// Return the most recent dynamic-loader error message, like `dlerror`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// `dlerror` wrapper.
pub fn dlerror_message() -> Result<Option<String>, FfiError> {
    Ok(ncl_sys::ffi::dlerror_message())
}

/// The C symbol name for a Lisp alien name, like `extern-alien-name`.
///
/// Phase 1 uses the Lisp name verbatim; the linkage-table mapping that would
/// rewrite it is the `update-alien-linkage-table` work, which needs `dlsym`.
#[must_use]
pub fn extern_alien_name(name: &str) -> String {
    name.to_owned()
}

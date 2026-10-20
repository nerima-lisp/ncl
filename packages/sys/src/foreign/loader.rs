use super::{ForeignError, SapAddress, call::ForeignFunction};
use core::ffi::{c_char, c_void};
use core::ptr::NonNull;
use std::path::Path;

/// An exact-path dynamic library handle closed when dropped.
#[derive(Debug)]
pub struct ForeignLibrary {
    handle: NonNull<c_void>,
}

// SAFETY: the handle is an immutable token owned by the system dynamic loader.
// POSIX loader entry points are thread-safe, and this wrapper calls only `dlsym`
// while borrowed and `dlclose` once from `Drop`. Arc ownership delays that close
// until every shared owner is gone. This does not make functions in the loaded
// library thread-safe; callers remain responsible for their own function contracts.
unsafe impl Send for ForeignLibrary {}
// SAFETY: concurrent symbol resolution uses the thread-safe loader API, while
// shared ownership prevents `Drop` from closing the handle during a borrow.
// The thread-safety of any resolved foreign function remains the caller's contract.
unsafe impl Sync for ForeignLibrary {}

impl ForeignLibrary {
    /// Load the library at an absolute path without searching fallback locations.
    ///
    /// # Errors
    ///
    /// Returns an error if the path is not absolute, the library cannot be loaded, or
    /// the platform does not support dynamic loading.
    pub fn load_exact(path: &Path) -> Result<Self, ForeignError> {
        #[cfg(unix)]
        {
            use std::ffi::CString;
            use std::os::unix::ffi::OsStrExt;

            if !path.is_absolute() {
                return Err(ForeignError::InvalidLibraryPath);
            }
            let path = CString::new(path.as_os_str().as_bytes())
                .map_err(|_| ForeignError::InvalidLibraryPath)?;
            // SAFETY: `dlerror` clears any stale thread-local loader diagnostic.
            let _ = unsafe { crate::os::declarations::dlerror() };
            // SAFETY: the path is a live, NUL-terminated absolute pathname. `RTLD_NOW`
            // requests immediate resolution and no search-path fallback is added here.
            let handle = unsafe { crate::os::declarations::dlopen(path.as_ptr(), 0x02) };
            NonNull::new(handle)
                .map(|handle| Self { handle })
                .ok_or(ForeignError::LibraryLoadFailed)
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            Err(ForeignError::UnsupportedPlatform)
        }
    }

    /// Resolve a symbol and retain this library's lifetime.
    ///
    /// # Errors
    ///
    /// Returns an error if the symbol cannot be resolved or dynamic loading is
    /// unsupported on this platform.
    pub fn symbol(&self, name: &core::ffi::CStr) -> Result<ForeignFunction<'_>, ForeignError> {
        #[cfg(unix)]
        {
            let address = self.resolve_symbol(name)?;
            Ok(ForeignFunction::from_address(address, self))
        }
        #[cfg(not(unix))]
        {
            let _ = name;
            Err(ForeignError::UnsupportedPlatform)
        }
    }

    /// Return the address of a function or data symbol in this library.
    ///
    /// # Errors
    ///
    /// Returns an error if the symbol cannot be resolved, its address does not fit
    /// the platform pointer width, or dynamic loading is unsupported.
    pub fn symbol_address(&self, name: &core::ffi::CStr) -> Result<SapAddress, ForeignError> {
        #[cfg(unix)]
        {
            let address = self.resolve_symbol(name)?;
            let raw = u64::try_from(address.as_ptr().expose_provenance())
                .map_err(|_| ForeignError::AddressOutOfRange)?;
            SapAddress::new(raw)
        }
        #[cfg(not(unix))]
        {
            let _ = name;
            Err(ForeignError::UnsupportedPlatform)
        }
    }

    #[cfg(unix)]
    fn resolve_symbol(&self, name: &core::ffi::CStr) -> Result<NonNull<c_void>, ForeignError> {
        // SAFETY: `dlerror` clears any prior thread-local loader diagnostic.
        let _ = unsafe { crate::os::declarations::dlerror() };
        // SAFETY: the handle is live through `self` and `name` is NUL-terminated.
        let address =
            unsafe { crate::os::declarations::dlsym(self.handle.as_ptr(), name.as_ptr()) };
        // SAFETY: `dlsym` completed on this thread; `dlerror` reports its thread-local result.
        let loader_error = unsafe { crate::os::declarations::dlerror() };
        classify_symbol_address(address, loader_error)
    }
}

/// Consume the calling thread's most recent dynamic-loader diagnostic.
///
/// # Errors
///
/// Returns an error if dynamic loading is unsupported on this platform.
pub fn take_loader_error() -> Result<Option<String>, ForeignError> {
    #[cfg(unix)]
    {
        // SAFETY: POSIX returns either null or a live, NUL-terminated diagnostic string.
        let message = unsafe { crate::os::declarations::dlerror() };
        if message.is_null() {
            return Ok(None);
        }
        // SAFETY: `dlerror` returned the valid diagnostic string described above; copy it now.
        let message = unsafe { core::ffi::CStr::from_ptr(message) };
        Ok(Some(message.to_string_lossy().into_owned()))
    }
    #[cfg(not(unix))]
    {
        Err(ForeignError::UnsupportedPlatform)
    }
}

fn classify_symbol_address(
    address: *mut c_void,
    loader_error: *const c_char,
) -> Result<NonNull<c_void>, ForeignError> {
    if !loader_error.is_null() {
        return Err(ForeignError::SymbolNotFound);
    }
    NonNull::new(address).ok_or(ForeignError::NullFunctionAddress)
}

impl Drop for ForeignLibrary {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            // SAFETY: this handle was returned by a successful `dlopen` and is closed once.
            let _ = unsafe { crate::os::declarations::dlclose(self.handle.as_ptr()) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_result_distinguishes_loader_error_from_null_address() {
        let null_address = core::ptr::null_mut();
        let loader_error = NonNull::<c_char>::dangling().as_ptr();
        assert_eq!(
            classify_symbol_address(null_address, loader_error),
            Err(ForeignError::SymbolNotFound)
        );
        assert_eq!(
            classify_symbol_address(null_address, core::ptr::null()),
            Err(ForeignError::NullFunctionAddress)
        );

        let function_address = NonNull::<c_void>::dangling().as_ptr();
        assert_eq!(
            classify_symbol_address(function_address, core::ptr::null()),
            NonNull::new(function_address).ok_or(ForeignError::NullFunctionAddress)
        );
    }

    #[cfg(unix)]
    #[test]
    fn loader_error_is_cleared_and_copied() {
        let result = ForeignLibrary::load_exact(Path::new(
            "/definitely/missing/ncl-foreign-library-for-dlerror-test.so",
        ));
        assert!(matches!(result, Err(ForeignError::LibraryLoadFailed)));
        assert!(matches!(take_loader_error(), Ok(Some(message)) if !message.is_empty()));
        assert_eq!(take_loader_error(), Ok(None));
    }
}

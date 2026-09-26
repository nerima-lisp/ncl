//! Checked conversion of native function pointers to code addresses.

/// An address could not be represented by the target integer type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FunctionAddressError;

impl std::fmt::Display for FunctionAddressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("native function address does not fit in u64")
    }
}

impl std::error::Error for FunctionAddressError {}

/// Convert a native function pointer to the integer representation consumed by
/// the code generator and builtin registry.
///
/// # Errors
///
/// Returns [`FunctionAddressError`] when the address cannot be represented as
/// a `u64` on the target platform.
pub fn function_address(function: *const ()) -> Result<u64, FunctionAddressError> {
    u64::try_from(function.addr()).map_err(|_| FunctionAddressError)
}

/// Convert a named native function to a checked code address.
#[macro_export]
macro_rules! function_address {
    ($function:path) => {
        $crate::function_address($function as *const ())
    };
}

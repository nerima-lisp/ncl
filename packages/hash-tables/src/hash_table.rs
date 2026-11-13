mod options;

pub use ncl_object::hash_table::{HashTable, HashTest, Weakness, sxhash};
pub use options::new_with_options;

use ncl_object::{ObjectError, Runtime};

/// Register the object layouts used by hash tables.
///
/// The table representation belongs to `ncl-object`, which registers its layouts before creating
/// the standard packages. This function remains as a compatibility entry point for callers that
/// register feature layouts explicitly.
///
/// # Errors
/// Returns an error if object layout registration fails.
pub fn register_layouts(runtime: &Runtime) -> Result<(), ObjectError> {
    runtime.register_layouts()
}

use ncl_object::{BuiltinIdentifier, BuiltinName, BuiltinPackage, ObjectError, Runtime, ThreadContext};

use crate::domain;

/// Register every implemented sequence builtin.
///
/// # Errors
///
/// Returns an error when a builtin cannot be installed.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    for line in include_str!("../ownership.tsv").lines().skip(1) {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() < 3 || fields[0] != "COMMON-LISP" || !fields[2].contains("function") {
            continue;
        }
        let name = fields[1];
        let Some(implementation) = domain::map::map_entry(name)
            .or_else(|| domain::set::set_entry(name))
            .or_else(|| domain::filter::filter_entry(name))
            .or_else(|| domain::sort::sort_entry(name))
        else {
            continue;
        };
        runtime.register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            implementation,
        )?;
    }
    Ok(())
}

use super::{BuiltinName, BuiltinPackage, ObjectError, Package, Runtime, ThreadContext};
use ncl_object::{make_string, with_root};
use ncl_packages::PackageOperations;

const SB_EXT_SYMBOLS: &[&str] = &[
    "*AFTER-GC-HOOKS*",
    "*GC-REAL-TIME*",
    "*GC-RUN-TIME*",
    "CANCEL-FINALIZATION",
    "FINALIZE",
    "GC",
    "GENERATION-BYTES-ALLOCATED",
    "HASH-TABLE-WEAKNESS",
    "MAKE-WEAK-POINTER",
    "MAKE-WEAK-VECTOR",
    "WEAK-POINTER",
    "WEAK-POINTER-P",
    "WEAK-POINTER-VALUE",
    "WEAK-VECTOR-P",
];
const NCL_GC_EXTERNAL_SYMBOLS: &[&str] = &["BYTES-CONSED-SINCE-GC", "GC-COUNT"];

/// Export NCL-GC counters and register the NCL-GC symbols that are also visible from SB-EXT.
///
/// Call after `ncl_object::register` has installed the NCL-GC symbols.
///
/// # Errors
/// Returns an object error when symbol lookup, package export, import, or function registration
/// fails.
pub fn register_gc_sb_ext_symbols(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) -> Result<(), ObjectError> {
    let mut source_package = runtime.ensure_package(ctx, BuiltinPackage::NclGc.as_str())?;
    with_root(ctx, &mut source_package, |ctx, source_package| {
        for &name in NCL_GC_EXTERNAL_SYMBOLS {
            let mut name_word = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
            with_root(ctx, &mut name_word, |ctx, name_word| {
                if !Package::from_word(*source_package).export(ctx, runtime, *name_word)? {
                    return Err(ObjectError::Layout);
                }
                Ok(())
            })?;
        }
        Ok(())
    })?;
    for &name in SB_EXT_SYMBOLS {
        crate::register_sb_symbol(
            ctx,
            runtime,
            BuiltinPackage::NclGc,
            BuiltinPackage::SbExt,
            BuiltinName::new(name),
            true,
        )?;
    }
    Ok(())
}

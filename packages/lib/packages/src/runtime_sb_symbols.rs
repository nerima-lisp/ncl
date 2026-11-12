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
        register_sb_symbol(
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

/// Import an existing NCL symbol into an SB package, exporting it when required.
///
/// This finds `name` in `source` without interning it, preserves the symbol's home package, and
/// copies an existing function registry entry to the SB package. SB-only surface rows remain the
/// responsibility of their owning domain crate.
///
/// # Errors
/// Returns `Unsupported` when `source` is not an NCL package, `target` is not an SB package, or
/// the source symbol is absent. Other object errors describe malformed runtime state.
///
/// # Panics
/// Panics if a root token cannot be removed in stack order.
pub fn register_sb_symbol(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: BuiltinPackage,
    target: BuiltinPackage,
    name: BuiltinName,
    external: bool,
) -> Result<(), ObjectError> {
    if !source.is_ncl() || !target.is_sb() {
        // check-added-lines: allow(unsupported) Preserve the API error for invalid package pairs.
        return Err(ObjectError::Unsupported);
    }
    let mut source_package = runtime.ensure_package(ctx, source.as_str())?;
    with_root(ctx, &mut source_package, |ctx, source_package| {
        let mut name_word = make_string(ctx, runtime, &name.as_str().chars().collect::<Vec<_>>())?;
        with_root(ctx, &mut name_word, |ctx, name_word| {
            let Some((mut symbol, _)) =
                Package::from_word(*source_package).find_symbol(ctx, runtime, *name_word)?
            else {
                // check-added-lines: allow(unsupported) The API rejects absent symbols without interning.
                return Err(ObjectError::Unsupported);
            };
            with_root(ctx, &mut symbol, |ctx, symbol| {
                let mut target_package = runtime.ensure_package(ctx, target.as_str())?;
                with_root(ctx, &mut target_package, |ctx, target_package| {
                    let package = Package::from_word(*target_package);
                    package.import(ctx, runtime, *name_word, *symbol)?;
                    if external {
                        package.export(ctx, runtime, *name_word)?;
                    }
                    if let Some(function) = runtime.function(ctx, source.as_str(), name.as_str()) {
                        let mut function = function;
                        with_root(ctx, &mut function, |ctx, function| {
                            runtime.define_function(ctx, target.as_str(), name.as_str(), *function)
                        })?;
                    }
                    Ok(())
                })
            })
        })
    })
}

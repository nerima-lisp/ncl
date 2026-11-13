use super::{BuiltinName, BuiltinPackage, ObjectError, Package, Runtime, ThreadContext};
use ncl_object::{make_string, with_root};
use ncl_packages::PackageOperations;

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

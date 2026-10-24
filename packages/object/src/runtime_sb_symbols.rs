use crate::{
    BuiltinName, BuiltinPackage, ObjectError, Package, Runtime, ThreadContext, make_string,
    with_root,
};

impl Runtime {
    /// Import an existing NCL symbol into an SB package, exporting it when required.
    ///
    /// This finds `name` in `source` without interning it, preserves the symbol's home package,
    /// and copies an existing function registry entry to the SB package. SB-only surface rows
    /// remain the responsibility of their owning domain crate.
    ///
    /// # Errors
    /// Returns `Unsupported` when `source` is not an NCL package, `target` is not an SB package,
    /// or the source symbol is absent. Other object errors describe malformed runtime state.
    ///
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn register_sb_symbol(
        &self,
        ctx: &mut ThreadContext,
        source: BuiltinPackage,
        target: BuiltinPackage,
        name: BuiltinName,
        external: bool,
    ) -> Result<(), ObjectError> {
        if !source.is_ncl() || !target.is_sb() {
            // check-added-lines: allow(unsupported) This API accepts only NCL-to-SB registrations.
            return Err(ObjectError::Unsupported);
        }
        let mut source_package = self.ensure_package(ctx, source.as_str())?;
        with_root(ctx, &mut source_package, |ctx, source_package| {
            let mut name_word = make_string(ctx, self, &name.as_str().chars().collect::<Vec<_>>())?;
            with_root(ctx, &mut name_word, |ctx, name_word| {
                let Some((mut symbol, _)) =
                    Package::from_word(*source_package).find_symbol(ctx, *name_word)?
                else {
                    // check-added-lines: allow(unsupported) Missing symbols must not be interned.
                    return Err(ObjectError::Unsupported);
                };
                with_root(ctx, &mut symbol, |ctx, symbol| {
                    let mut target_package = self.ensure_package(ctx, target.as_str())?;
                    with_root(ctx, &mut target_package, |ctx, target_package| {
                        let package = Package::from_word(*target_package);
                        package.import(ctx, self, *name_word, *symbol)?;
                        if external {
                            package.export(ctx, self, *name_word)?;
                        }
                        if let Some(function) = self.function(ctx, source.as_str(), name.as_str()) {
                            let mut function = function;
                            with_root(ctx, &mut function, |ctx, function| {
                                self.define_function(ctx, target.as_str(), name.as_str(), *function)
                            })?;
                        }
                        Ok(())
                    })
                })
            })
        })
    }
}

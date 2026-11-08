use ncl_object::{ObjectError, Package, Runtime, ThreadContext, Word, car, cdr, put};
use ncl_object::package::{NAME, NICKNAMES};

/// CLHS operations that update the runtime's package registry.
pub trait RuntimePackageOperations {
    /// Rename a registered package and replace its nickname list.
    ///
    /// # Errors
    /// Returns [`ObjectError`] if the package, name, nicknames, or registry is invalid.
    fn rename_package(
        &self,
        ctx: &mut ThreadContext,
        package: Package,
        name: Word,
        nicknames: Word,
    ) -> Result<(), ObjectError>;

    /// Remove a package from the runtime registry.
    ///
    /// # Errors
    /// Returns [`ObjectError`] if package metadata or registry state is malformed.
    fn delete_package(
        &self,
        ctx: &mut ThreadContext,
        package: Package,
    ) -> Result<bool, ObjectError>;
}

impl RuntimePackageOperations for Runtime {
    fn rename_package(
        &self,
        ctx: &mut ThreadContext,
        package: Package,
        name: Word,
        nicknames: Word,
    ) -> Result<(), ObjectError> {
        let mut package_word = package.as_word();
        ncl_object::with_root(ctx, &mut package_word, |ctx, package| {
            Package::from_word(*package).ensure_unlocked(ctx)?;
            let mut name = name;
            ncl_object::with_root(ctx, &mut name, |ctx, name| {
                ncl_object::string_length(ctx, *name)?;
                let mut nicknames = nicknames;
                ncl_object::with_root(ctx, &mut nicknames, |ctx, nicknames| {
                    let mut names = *nicknames;
                    while names != Word::NIL {
                        let nickname = car(ctx, names)?;
                        ncl_object::string_length(ctx, nickname)?;
                        self.check_package_name(ctx, *package, nickname)?;
                        names = cdr(ctx, names)?;
                    }
                    self.check_package_name(ctx, *package, *name)?;

                    let old_name = Package::from_word(*package).name(ctx)?;
                    self.unregister_package_name(ctx, old_name)?;
                    let mut old_nicknames = Package::from_word(*package).nicknames(ctx)?;
                    while old_nicknames != Word::NIL {
                        let nickname = car(ctx, old_nicknames)?;
                        self.unregister_package_name(ctx, nickname)?;
                        old_nicknames = cdr(ctx, old_nicknames)?;
                    }
                    put(ctx, *package, NAME, *name)?;
                    put(ctx, *package, NICKNAMES, *nicknames)?;
                    self.register_package_name(ctx, *package, *name, false)?;
                    let mut names = *nicknames;
                    while names != Word::NIL {
                        self.register_package_name(ctx, *package, car(ctx, names)?, true)?;
                        names = cdr(ctx, names)?;
                    }
                    Ok(())
                })
            })
        })
    }

    fn delete_package(
        &self,
        ctx: &mut ThreadContext,
        package: Package,
    ) -> Result<bool, ObjectError> {
        let mut package_word = package.as_word();
        ncl_object::with_root(ctx, &mut package_word, |ctx, package| {
            Package::from_word(*package).ensure_unlocked(ctx)?;
            let mut name = Package::from_word(*package).name(ctx)?;
            if name == Word::NIL {
                return Ok(false);
            }
            ncl_object::with_root(ctx, &mut name, |ctx, name| {
                let mut nicknames = Package::from_word(*package).nicknames(ctx)?;
                let mut removed = self.unregister_package_name(ctx, *name)?;
                while nicknames != Word::NIL {
                    let mut nickname = car(ctx, nicknames)?;
                    ncl_object::with_root(ctx, &mut nickname, |ctx, nickname| {
                        removed |= self.unregister_package_name(ctx, *nickname)?;
                        Ok(())
                    })?;
                    nicknames = cdr(ctx, nicknames)?;
                }
                if removed {
                    put(ctx, *package, NAME, Word::NIL)?;
                    self.release_package_symbols(*package)?;
                }
                Ok(removed)
            })
        })
    }
}

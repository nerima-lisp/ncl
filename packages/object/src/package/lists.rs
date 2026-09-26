use super::{EXTERNAL, INTERNAL, NICKNAMES, SHADOWING, USE_LIST, USED_BY};
use crate::hash_table::HashTable;
use crate::object_access::{get, put};
use crate::widetag;
use crate::{ObjectError, Runtime, ThreadContext, make_cons, rplacd, symbol_name};
use ncl_sys::Word;

impl super::Package {
    /// Add another package to this package's use list.
    ///
    /// # Errors
    /// Returns a layout error when a package list is malformed.
    ///
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn use_package(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        package: Word,
    ) -> Result<bool, ObjectError> {
        let mut package_self = self.0;
        crate::with_root(ctx, &mut package_self, |ctx, package_self| {
            Self::from_word(*package_self).ensure_unlocked(ctx)?;
            let mut package = package;
            crate::with_root(ctx, &mut package, |ctx, package| {
                Self::from_word(*package).ensure_unlocked(ctx)?;
                let mut list = get(ctx, *package_self, widetag::PACKAGE, USE_LIST)?;
                while list != Word::NIL {
                    if ncl_sys::read_cons_word(&ctx.thread, list, 0) == Some(*package) {
                        return Ok(false);
                    }
                    list =
                        ncl_sys::read_cons_word(&ctx.thread, list, 1).ok_or(ObjectError::Layout)?;
                }
                let list = make_cons(
                    ctx,
                    runtime,
                    *package,
                    get(ctx, *package_self, widetag::PACKAGE, USE_LIST)?,
                )?;
                put(ctx, *package_self, USE_LIST, list)?;
                let mut used_by = get(ctx, *package, widetag::PACKAGE, USED_BY)?;
                while used_by != Word::NIL {
                    if ncl_sys::read_cons_word(&ctx.thread, used_by, 0) == Some(*package_self) {
                        return Ok(true);
                    }
                    used_by = ncl_sys::read_cons_word(&ctx.thread, used_by, 1)
                        .ok_or(ObjectError::Layout)?;
                }
                let used_by = make_cons(
                    ctx,
                    runtime,
                    *package_self,
                    get(ctx, *package, widetag::PACKAGE, USED_BY)?,
                )?;
                put(ctx, *package, USED_BY, used_by)?;
                Ok(true)
            })
        })
    }

    /// Remove another package from this package's use list.
    ///
    /// # Errors
    /// Returns a layout error when a package list is malformed.
    ///
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn unuse_package(
        self,
        ctx: &mut ThreadContext,
        package: Word,
    ) -> Result<bool, ObjectError> {
        let mut package_self = self.0;
        crate::with_root(ctx, &mut package_self, |ctx, package_self| {
            Self::from_word(*package_self).ensure_unlocked(ctx)?;
            let mut package = package;
            crate::with_root(ctx, &mut package, |ctx, package| {
                Self::from_word(*package).ensure_unlocked(ctx)?;
                let removed = remove_from_list(ctx, *package_self, USE_LIST, *package)?;
                if !removed {
                    return Ok(false);
                }
                remove_from_list(ctx, *package, USED_BY, *package_self)?;
                Ok(true)
            })
        })
    }

    /// Remove a symbol from internal or external visibility.
    ///
    /// # Errors
    /// Returns a layout error when a package or shadowing list is malformed.
    ///
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn unintern(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
    ) -> Result<bool, ObjectError> {
        let mut package = self.0;
        crate::with_root(ctx, &mut package, |ctx, package| {
            Self::from_word(*package).ensure_unlocked(ctx)?;
            let mut name = name;
            crate::with_root(ctx, &mut name, |ctx, name| {
                let symbol =
                    match HashTable::from_word(get(ctx, *package, widetag::PACKAGE, INTERNAL)?)
                        .remove(ctx, runtime, *name)?
                    {
                        Some(symbol) => Some(symbol),
                        None => {
                            HashTable::from_word(get(ctx, *package, widetag::PACKAGE, EXTERNAL)?)
                                .remove(ctx, runtime, *name)?
                        }
                    };
                let Some(mut symbol) = symbol else {
                    return Ok(false);
                };
                crate::with_root(ctx, &mut symbol, |ctx, symbol| {
                    if get(
                        ctx,
                        *symbol,
                        widetag::SYMBOL,
                        crate::layout::symbol_offset::PACKAGE,
                    )? == *package
                    {
                        put(
                            ctx,
                            *symbol,
                            crate::layout::symbol_offset::PACKAGE,
                            Word::NIL,
                        )?;
                    }
                    let mut current = get(ctx, *package, widetag::PACKAGE, SHADOWING)?;
                    let mut previous = Word::NIL;
                    while current != Word::NIL {
                        let next = ncl_sys::read_cons_word(&ctx.thread, current, 1)
                            .ok_or(ObjectError::Layout)?;
                        if ncl_sys::read_cons_word(&ctx.thread, current, 0) == Some(*symbol) {
                            if previous == Word::NIL {
                                put(ctx, *package, SHADOWING, next)?;
                            } else {
                                rplacd(ctx, previous, next)?;
                            }
                            break;
                        }
                        previous = current;
                        current = next;
                    }
                    Ok(true)
                })
            })
        })
    }

    /// Remove a nickname from this package.
    ///
    /// # Errors
    /// Returns a package error when this package is locked, or a layout error
    /// when the nickname list is malformed.
    pub fn remove_nickname(
        self,
        ctx: &mut ThreadContext,
        nickname: Word,
    ) -> Result<bool, ObjectError> {
        let mut package = self.0;
        crate::with_root(ctx, &mut package, |ctx, package| {
            Self::from_word(*package).ensure_unlocked(ctx)?;
            let mut nickname = nickname;
            crate::with_root(ctx, &mut nickname, |ctx, nickname| {
                remove_from_list(ctx, *package, NICKNAMES, *nickname)
            })
        })
    }

    /// Add a name to the package's shadowing list.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    ///
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn shadow(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
    ) -> Result<(), ObjectError> {
        let mut package = self.0;
        crate::with_root(ctx, &mut package, |ctx, package| {
            Self::from_word(*package).ensure_unlocked(ctx)?;
            let mut name = name;
            crate::with_root(ctx, &mut name, |ctx, name| {
                let (mut symbol, new_symbol) =
                    if let Some((symbol, _)) = Self::from_word(*package).find_symbol(ctx, *name)? {
                        (symbol, false)
                    } else {
                        (crate::make_symbol(ctx, runtime, *name)?, true)
                    };
                crate::with_root(ctx, &mut symbol, |ctx, symbol| {
                    if new_symbol {
                        put(
                            ctx,
                            *symbol,
                            crate::layout::symbol_offset::PACKAGE,
                            *package,
                        )?;
                        HashTable::from_word(get(ctx, *package, widetag::PACKAGE, INTERNAL)?)
                            .insert(ctx, runtime, *name, *symbol)?;
                    }
                    let mut list = get(ctx, *package, widetag::PACKAGE, SHADOWING)?;
                    while list != Word::NIL {
                        if ncl_sys::read_cons_word(&ctx.thread, list, 0) == Some(*symbol) {
                            return Ok(());
                        }
                        list = ncl_sys::read_cons_word(&ctx.thread, list, 1)
                            .ok_or(ObjectError::Layout)?;
                    }
                    let list = make_cons(
                        ctx,
                        runtime,
                        *symbol,
                        get(ctx, *package, widetag::PACKAGE, SHADOWING)?,
                    )?;
                    put(ctx, *package, SHADOWING, list)
                })
            })
        })
    }

    /// Import a symbol and record it in this package's shadowing list.
    ///
    /// An accessible symbol with the same name is first uninterned so the
    /// imported symbol wins lookup in this package.
    ///
    /// # Errors
    /// Returns an object error when the package is locked, the symbol is
    /// malformed, or the package tables cannot be updated.
    pub fn shadowing_import(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        symbol: Word,
    ) -> Result<(), ObjectError> {
        self.ensure_unlocked(ctx)?;
        let mut package = self.0;
        crate::with_root(ctx, &mut package, |ctx, package| {
            let mut symbol = symbol;
            crate::with_root(ctx, &mut symbol, |ctx, symbol| {
                let mut name = symbol_name(ctx, *symbol)?;
                crate::with_root(ctx, &mut name, |ctx, name| {
                    Self::from_word(*package).unintern(ctx, runtime, *name)?;
                    Self::from_word(*package).import(ctx, runtime, *name, *symbol)?;
                    let mut list = get(ctx, *package, widetag::PACKAGE, SHADOWING)?;
                    while list != Word::NIL {
                        if ncl_sys::read_cons_word(&ctx.thread, list, 0) == Some(*symbol) {
                            return Ok(());
                        }
                        list = ncl_sys::read_cons_word(&ctx.thread, list, 1)
                            .ok_or(ObjectError::Layout)?;
                    }
                    let list = make_cons(
                        ctx,
                        runtime,
                        *symbol,
                        get(ctx, *package, widetag::PACKAGE, SHADOWING)?,
                    )?;
                    put(ctx, *package, SHADOWING, list)
                })
            })
        })
    }
}

fn remove_from_list(
    ctx: &mut ThreadContext,
    object: Word,
    slot: usize,
    target: Word,
) -> Result<bool, ObjectError> {
    let mut current = get(ctx, object, widetag::PACKAGE, slot)?;
    let mut previous = Word::NIL;
    while current != Word::NIL {
        let next = ncl_sys::read_cons_word(&ctx.thread, current, 1).ok_or(ObjectError::Layout)?;
        if ncl_sys::read_cons_word(&ctx.thread, current, 0) == Some(target) {
            if previous == Word::NIL {
                put(ctx, object, slot, next)?;
            } else {
                rplacd(ctx, previous, next)?;
            }
            return Ok(true);
        }
        previous = current;
        current = next;
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LispError, PackageError};

    fn setup() -> (Runtime, ThreadContext, super::super::Package) {
        let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("register: {error:?}"));
        let package = super::super::Package::new(&mut ctx, &runtime, "LOCKED-LISTS")
            .unwrap_or_else(|error| panic!("package: {error:?}"));
        (runtime, ctx, package)
    }

    #[test]
    fn locked_shadow_rejects_with_typed_package_error() {
        let (runtime, mut ctx, package) = setup();
        let name = crate::make_string(&mut ctx, &runtime, &['N'])
            .unwrap_or_else(|error| panic!("name: {error:?}"));
        package
            .set_locked(&mut ctx, true)
            .unwrap_or_else(|error| panic!("lock: {error:?}"));

        assert_eq!(
            package.shadow(&mut ctx, &runtime, name),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            ctx.take_pending_lisp_error(),
            Some(LispError::PackageError(PackageError::Locked))
        );
    }

    #[test]
    fn locked_use_package_rejects_with_typed_package_error() {
        let (runtime, mut ctx, package) = setup();
        let source = super::super::Package::new(&mut ctx, &runtime, "SOURCE-LISTS")
            .unwrap_or_else(|error| panic!("source: {error:?}"));
        package
            .set_locked(&mut ctx, true)
            .unwrap_or_else(|error| panic!("lock: {error:?}"));

        assert_eq!(
            package.use_package(&mut ctx, &runtime, source.as_word()),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            ctx.take_pending_lisp_error(),
            Some(LispError::PackageError(PackageError::Locked))
        );
    }

    #[test]
    fn locked_unuse_package_rejects_with_typed_package_error() {
        let (runtime, mut ctx, package) = setup();
        let source = super::super::Package::new(&mut ctx, &runtime, "SOURCE-LISTS")
            .unwrap_or_else(|error| panic!("source: {error:?}"));
        package
            .use_package(&mut ctx, &runtime, source.as_word())
            .unwrap_or_else(|error| panic!("use setup: {error:?}"));
        package
            .set_locked(&mut ctx, true)
            .unwrap_or_else(|error| panic!("lock: {error:?}"));

        assert_eq!(
            package.unuse_package(&mut ctx, source.as_word()),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            ctx.take_pending_lisp_error(),
            Some(LispError::PackageError(PackageError::Locked))
        );
    }

    #[test]
    fn locked_target_rejects_use_package_with_typed_package_error() {
        let (runtime, mut ctx, package) = setup();
        let target = super::super::Package::new(&mut ctx, &runtime, "TARGET-LISTS")
            .unwrap_or_else(|error| panic!("target: {error:?}"));
        target
            .set_locked(&mut ctx, true)
            .unwrap_or_else(|error| panic!("lock: {error:?}"));

        assert_eq!(
            package.use_package(&mut ctx, &runtime, target.as_word()),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            ctx.take_pending_lisp_error(),
            Some(LispError::PackageError(PackageError::Locked))
        );
    }

    #[test]
    fn locked_target_rejects_unuse_package_with_typed_package_error() {
        let (runtime, mut ctx, package) = setup();
        let target = super::super::Package::new(&mut ctx, &runtime, "TARGET-LISTS")
            .unwrap_or_else(|error| panic!("target: {error:?}"));
        package
            .use_package(&mut ctx, &runtime, target.as_word())
            .unwrap_or_else(|error| panic!("use setup: {error:?}"));
        target
            .set_locked(&mut ctx, true)
            .unwrap_or_else(|error| panic!("lock: {error:?}"));

        assert_eq!(
            package.unuse_package(&mut ctx, target.as_word()),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            ctx.take_pending_lisp_error(),
            Some(LispError::PackageError(PackageError::Locked))
        );
    }
}

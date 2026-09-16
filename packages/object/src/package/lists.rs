use super::{EXTERNAL, INTERNAL, SHADOWING, USE_LIST, USED_BY};
use crate::hash_table::HashTable;
use crate::object_access::{get, put};
use crate::widetag;
use crate::{ObjectError, Runtime, ThreadContext, make_cons, rplacd};
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
            let mut package = package;
            crate::with_root(ctx, &mut package, |ctx, package| {
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
            let mut package = package;
            crate::with_root(ctx, &mut package, |ctx, package| {
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
            let mut name = name;
            crate::with_root(ctx, &mut name, |ctx, name| {
                let symbol = match HashTable::from(get(ctx, *package, widetag::PACKAGE, INTERNAL)?)
                    .remove(ctx, runtime, *name)?
                {
                    Some(symbol) => Some(symbol),
                    None => HashTable::from(get(ctx, *package, widetag::PACKAGE, EXTERNAL)?)
                        .remove(ctx, runtime, *name)?,
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
            let mut name = name;
            crate::with_root(ctx, &mut name, |ctx, name| {
                let symbol =
                    if let Some((symbol, _)) = Self::from(*package).find_symbol(ctx, *name)? {
                        symbol
                    } else {
                        let mut symbol = crate::make_symbol(ctx, runtime, *name)?;
                        crate::with_root(ctx, &mut symbol, |ctx, symbol| {
                            put(
                                ctx,
                                *symbol,
                                crate::layout::symbol_offset::PACKAGE,
                                *package,
                            )?;
                            HashTable::from(get(ctx, *package, widetag::PACKAGE, INTERNAL)?)
                                .insert(ctx, runtime, *name, *symbol)?;
                            Ok(*symbol)
                        })?
                    };
                let mut list = get(ctx, *package, widetag::PACKAGE, SHADOWING)?;
                while list != Word::NIL {
                    if ncl_sys::read_cons_word(&ctx.thread, list, 0) == Some(symbol) {
                        return Ok(());
                    }
                    list =
                        ncl_sys::read_cons_word(&ctx.thread, list, 1).ok_or(ObjectError::Layout)?;
                }
                let list = make_cons(
                    ctx,
                    runtime,
                    symbol,
                    get(ctx, *package, widetag::PACKAGE, SHADOWING)?,
                )?;
                put(ctx, *package, SHADOWING, list)
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

use crate::{PackageListOperations, PackageOperations};
use ncl_object::package::{NICKNAMES, SHADOWING, USE_LIST, USED_BY};
use ncl_object::widetag;
use ncl_object::{
    ObjectError, Package, Runtime, ThreadContext, Word, car, cdr, get, make_cons, make_symbol, put,
    rplacd, symbol_name,
};

impl PackageListOperations for Package {
    fn use_package(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        package: Word,
    ) -> Result<bool, ObjectError> {
        let mut package_self = self.as_word();
        ncl_object::with_root(ctx, &mut package_self, |ctx, package_self| {
            Self::from_word(*package_self).ensure_unlocked(ctx)?;
            let mut package = package;
            ncl_object::with_root(ctx, &mut package, |ctx, package| {
                Self::from_word(*package).ensure_unlocked(ctx)?;
                let mut list = get(ctx, *package_self, widetag::PACKAGE, USE_LIST)?;
                while list != Word::NIL {
                    if car(ctx, list)? == *package {
                        return Ok(false);
                    }
                    list = cdr(ctx, list)?;
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
                    if car(ctx, used_by)? == *package_self {
                        return Ok(true);
                    }
                    used_by = cdr(ctx, used_by)?;
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

    fn unuse_package(self, ctx: &mut ThreadContext, package: Word) -> Result<bool, ObjectError> {
        let mut package_self = self.as_word();
        ncl_object::with_root(ctx, &mut package_self, |ctx, package_self| {
            Self::from_word(*package_self).ensure_unlocked(ctx)?;
            let mut package = package;
            ncl_object::with_root(ctx, &mut package, |ctx, package| {
                Self::from_word(*package).ensure_unlocked(ctx)?;
                let removed = remove_from_list(ctx, *package_self, USE_LIST, *package)?;
                if removed {
                    remove_from_list(ctx, *package, USED_BY, *package_self)?;
                }
                Ok(removed)
            })
        })
    }

    fn unintern(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
    ) -> Result<bool, ObjectError> {
        let mut package = self.as_word();
        ncl_object::with_root(ctx, &mut package, |ctx, package| {
            Self::from_word(*package).ensure_unlocked(ctx)?;
            let mut name = name;
            ncl_object::with_root(ctx, &mut name, |ctx, name| {
                let symbol = match Self::from_word(*package)
                    .remove_table_symbol(ctx, runtime, false, *name)?
                {
                    Some(symbol) => Some(symbol),
                    None => Self::from_word(*package)
                        .remove_table_symbol(ctx, runtime, true, *name)?,
                };
                let Some(mut symbol) = symbol else {
                    return Ok(false);
                };
                ncl_object::with_root(ctx, &mut symbol, |ctx, symbol| {
                    if get(
                        ctx,
                        *symbol,
                        widetag::SYMBOL,
                        ncl_object::layout::symbol_offset::PACKAGE,
                    )? == *package
                    {
                        put(
                            ctx,
                            *symbol,
                            ncl_object::layout::symbol_offset::PACKAGE,
                            Word::NIL,
                        )?;
                    }
                    remove_from_list(ctx, *package, SHADOWING, *symbol)?;
                    Ok(true)
                })
            })
        })
    }

    fn remove_nickname(self, ctx: &mut ThreadContext, nickname: Word) -> Result<bool, ObjectError> {
        let mut package = self.as_word();
        ncl_object::with_root(ctx, &mut package, |ctx, package| {
            Self::from_word(*package).ensure_unlocked(ctx)?;
            let mut nickname = nickname;
            ncl_object::with_root(ctx, &mut nickname, |ctx, nickname| {
                remove_from_list(ctx, *package, NICKNAMES, *nickname)
            })
        })
    }

    fn shadow(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
    ) -> Result<(), ObjectError> {
        let mut package = self.as_word();
        ncl_object::with_root(ctx, &mut package, |ctx, package| {
            Self::from_word(*package).ensure_unlocked(ctx)?;
            let mut name = name;
            ncl_object::with_root(ctx, &mut name, |ctx, name| {
                let (mut symbol, new_symbol) = if let Some((symbol, _)) =
                    Self::from_word(*package).find_symbol(ctx, runtime, *name)?
                {
                    (symbol, false)
                } else {
                    (make_symbol(ctx, runtime, *name)?, true)
                };
                ncl_object::with_root(ctx, &mut symbol, |ctx, symbol| {
                    if new_symbol {
                        put(
                            ctx,
                            *symbol,
                            ncl_object::layout::symbol_offset::PACKAGE,
                            *package,
                        )?;
                        Self::from_word(*package)
                            .insert_table_symbol(ctx, runtime, false, *name, *symbol)?;
                    }
                    let mut list = get(ctx, *package, widetag::PACKAGE, SHADOWING)?;
                    while list != Word::NIL {
                        if car(ctx, list)? == *symbol {
                            return Ok(());
                        }
                        list = cdr(ctx, list)?;
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

    fn shadowing_import(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        symbol: Word,
    ) -> Result<(), ObjectError> {
        self.ensure_unlocked(ctx)?;
        let mut package = self.as_word();
        ncl_object::with_root(ctx, &mut package, |ctx, package| {
            let mut symbol = symbol;
            ncl_object::with_root(ctx, &mut symbol, |ctx, symbol| {
                let mut name = symbol_name(ctx, *symbol)?;
                ncl_object::with_root(ctx, &mut name, |ctx, name| {
                    Self::from_word(*package).unintern(ctx, runtime, *name)?;
                    Self::from_word(*package).import(ctx, runtime, *name, *symbol)?;
                    let mut list = get(ctx, *package, widetag::PACKAGE, SHADOWING)?;
                    while list != Word::NIL {
                        if car(ctx, list)? == *symbol {
                            return Ok(());
                        }
                        list = cdr(ctx, list)?;
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
        let next = cdr(ctx, current)?;
        if car(ctx, current)? == target {
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

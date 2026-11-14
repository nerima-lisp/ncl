use crate::{PackageListOperations, PackageOperations};
use ncl_hash_tables::HashTable;
use ncl_object::package::{EXTERNAL, INTERNAL, NICKNAMES, SHADOWING, USE_LIST, USED_BY};
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
                    if list_car(ctx, list) == Some(*package) {
                        return Ok(false);
                    }
                    list = list_cdr(ctx, list)?;
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
                    if list_car(ctx, used_by) == Some(*package_self) {
                        return Ok(true);
                    }
                    used_by = list_cdr(ctx, used_by)?;
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
                if !removed {
                    return Ok(false);
                }
                remove_from_list(ctx, *package, USED_BY, *package_self)?;
                Ok(true)
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
                let internal =
                    HashTable::from_word(get(ctx, *package, widetag::PACKAGE, INTERNAL)?);
                let symbol = match internal.remove(ctx, runtime, *name)? {
                    Some(symbol) => Some(symbol),
                    None => HashTable::from_word(get(ctx, *package, widetag::PACKAGE, EXTERNAL)?)
                        .remove(ctx, runtime, *name)?,
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
                    let mut current = get(ctx, *package, widetag::PACKAGE, SHADOWING)?;
                    let mut previous = Word::NIL;
                    while current != Word::NIL {
                        let next = list_cdr(ctx, current)?;
                        if list_car(ctx, current) == Some(*symbol) {
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

    fn remove_nickname(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        nickname: Word,
    ) -> Result<bool, ObjectError> {
        let mut package = self.as_word();
        ncl_object::with_root(ctx, &mut package, |ctx, package| {
            Self::from_word(*package).ensure_unlocked(ctx)?;
            let mut nickname = nickname;
            ncl_object::with_root(ctx, &mut nickname, |ctx, nickname| {
                let (present, another_equal_nickname) =
                    nickname_removal_state(ctx, *package, *nickname)?;
                if !present {
                    return Ok(false);
                }
                if !another_equal_nickname {
                    runtime.unregister_package_name_for_package(ctx, *package, *nickname)?;
                }
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
                let (mut symbol, new_symbol) =
                    if let Some((symbol, _)) = Self::from_word(*package).find_symbol(ctx, *name)? {
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
                        HashTable::from_word(get(ctx, *package, widetag::PACKAGE, INTERNAL)?)
                            .insert(ctx, runtime, *name, *symbol)?;
                    }
                    let mut list = get(ctx, *package, widetag::PACKAGE, SHADOWING)?;
                    while list != Word::NIL {
                        if list_car(ctx, list) == Some(*symbol) {
                            return Ok(());
                        }
                        list = list_cdr(ctx, list)?;
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
                        if list_car(ctx, list) == Some(*symbol) {
                            return Ok(());
                        }
                        list = list_cdr(ctx, list)?;
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
        let next = list_cdr(ctx, current)?;
        if list_car(ctx, current) == Some(target) {
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

fn nickname_removal_state(
    ctx: &ThreadContext,
    package: Word,
    target: Word,
) -> Result<(bool, bool), ObjectError> {
    let mut current = get(ctx, package, widetag::PACKAGE, NICKNAMES)?;
    let mut found_target = false;
    let mut another_equal_nickname = false;
    while current != Word::NIL {
        let nickname = list_car(ctx, current).ok_or(ObjectError::Layout)?;
        if nickname == target && !found_target {
            found_target = true;
        } else if names_equal(ctx, nickname, target)? {
            another_equal_nickname = true;
        }
        current = list_cdr(ctx, current)?;
    }
    Ok((found_target, another_equal_nickname))
}

fn names_equal(ctx: &ThreadContext, left: Word, right: Word) -> Result<bool, ObjectError> {
    if left == right {
        return Ok(true);
    }
    if ncl_object::object_widetag(ctx, left) != Some(widetag::STRING)
        || ncl_object::object_widetag(ctx, right) != Some(widetag::STRING)
    {
        return Ok(false);
    }
    let length = ncl_object::string_length(ctx, left)?;
    if length != ncl_object::string_length(ctx, right)? {
        return Ok(false);
    }
    for index in 0..length {
        if ncl_object::string_ref(ctx, left, index)? != ncl_object::string_ref(ctx, right, index)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn list_car(ctx: &ThreadContext, list: Word) -> Option<Word> {
    car(ctx, list).ok()
}

fn list_cdr(ctx: &ThreadContext, list: Word) -> Result<Word, ObjectError> {
    cdr(ctx, list).map_err(|_| ObjectError::Layout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_use_list_returns_layout_error() {
        let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("register: {error:?}"));
        let package = Package::new(&mut ctx, &runtime, "MALFORMED-USE-LIST")
            .unwrap_or_else(|error| panic!("package: {error:?}"));
        let source = Package::new(&mut ctx, &runtime, "MALFORMED-USE-SOURCE")
            .unwrap_or_else(|error| panic!("source: {error:?}"));
        put(&mut ctx, package.as_word(), USE_LIST, Word::TRUE)
            .unwrap_or_else(|error| panic!("malformed use list: {error:?}"));

        assert_eq!(
            package.use_package(&mut ctx, &runtime, source.as_word()),
            Err(ObjectError::Layout)
        );
    }
}

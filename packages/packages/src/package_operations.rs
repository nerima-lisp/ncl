use crate::PackageOperations;
use ncl_object::{
    ObjectError, Package, Runtime, ThreadContext, Word, get, make_cons, put,
};
use ncl_object::package::{EXTERNAL, INTERNAL, NICKNAMES};
use ncl_object::widetag;
use std::collections::HashSet;

impl PackageOperations for Package {
    fn for_each_symbol<F>(self, ctx: &ThreadContext, mut visit: F) -> Result<(), ObjectError>
    where
        F: FnMut(Word),
    {
        for slot in [INTERNAL, EXTERNAL] {
            let mut entries = get(ctx, self.as_word(), widetag::PACKAGE, slot)?;
            let mut seen = HashSet::new();
            while entries != Word::NIL {
                if !seen.insert(entries) {
                    return Err(ObjectError::Layout);
                }
                visit(ncl_object::car(ctx, entries)?);
                entries = ncl_object::cdr(ctx, entries)?;
            }
        }
        Ok(())
    }

    fn add_nickname(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        nickname: Word,
    ) -> Result<bool, ObjectError> {
        let mut package = self.as_word();
        ncl_object::with_root(ctx, &mut package, |ctx, package| {
            Package::from_word(*package).ensure_unlocked(ctx)?;
            let mut nickname = nickname;
            ncl_object::with_root(ctx, &mut nickname, |ctx, nickname| {
                let mut names = get(ctx, *package, widetag::PACKAGE, NICKNAMES)?;
                while names != Word::NIL {
                    if ncl_object::car(ctx, names)? == *nickname {
                        return Ok(false);
                    }
                    names = ncl_object::cdr(ctx, names)?;
                }
                let names = make_cons(
                    ctx,
                    runtime,
                    *nickname,
                    get(ctx, *package, widetag::PACKAGE, NICKNAMES)?,
                )?;
                put(ctx, *package, NICKNAMES, names)?;
                Ok(true)
            })
        })
    }

    fn export(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
    ) -> Result<bool, ObjectError> {
        let mut package = self.as_word();
        ncl_object::with_root(ctx, &mut package, |ctx, package| {
            Package::from_word(*package).ensure_unlocked(ctx)?;
            let mut name = name;
            ncl_object::with_root(ctx, &mut name, |ctx, name| {
                let mut symbol = if let Some(symbol) = Package::from_word(*package)
                    .remove_table_symbol(ctx, runtime, false, *name)?
                {
                    symbol
                } else {
                    if Package::from_word(*package)
                        .table_symbol(ctx, runtime, true, *name)?
                        .is_some()
                    {
                        return Ok(true);
                    }
                    let Some((symbol, ncl_object::FindStatus::Inherited)) =
                        Package::from_word(*package).find_symbol(ctx, runtime, *name)?
                    else {
                        return Ok(false);
                    };
                    Package::from_word(*package).import(ctx, runtime, *name, symbol)?;
                    Package::from_word(*package)
                        .remove_table_symbol(ctx, runtime, false, *name)?
                        .ok_or(ObjectError::Layout)?
                };
                ncl_object::with_root(ctx, &mut symbol, |ctx, symbol| {
                    Package::from_word(*package)
                        .insert_table_symbol(ctx, runtime, true, *name, *symbol)
                })?;
                Ok(true)
            })
        })
    }

    fn unexport(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
    ) -> Result<bool, ObjectError> {
        let mut package = self.as_word();
        ncl_object::with_root(ctx, &mut package, |ctx, package| {
            Package::from_word(*package).ensure_unlocked(ctx)?;
            let mut name = name;
            ncl_object::with_root(ctx, &mut name, |ctx, name| {
                let Some(mut symbol) =
                    Package::from_word(*package).remove_table_symbol(ctx, runtime, true, *name)?
                else {
                    return Ok(false);
                };
                ncl_object::with_root(ctx, &mut symbol, |ctx, symbol| {
                    Package::from_word(*package)
                        .insert_table_symbol(ctx, runtime, false, *name, *symbol)
                })?;
                Ok(true)
            })
        })
    }

    fn import(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
        symbol: Word,
    ) -> Result<(), ObjectError> {
        let mut package = self.as_word();
        ncl_object::with_root(ctx, &mut package, |ctx, package| {
            Package::from_word(*package).ensure_unlocked(ctx)?;
            let mut name = name;
            ncl_object::with_root(ctx, &mut name, |ctx, name| {
                let mut symbol = symbol;
                ncl_object::with_root(ctx, &mut symbol, |ctx, symbol| {
                    let home = get(
                        ctx,
                        *symbol,
                        widetag::SYMBOL,
                        ncl_object::layout::symbol_offset::PACKAGE,
                    )?;
                    if let Some((existing, status)) =
                        Package::from_word(*package).find_symbol(ctx, runtime, *name)?
                    {
                        if existing == *symbol {
                            if status != ncl_object::FindStatus::Inherited {
                                return Ok(());
                            }
                        } else {
                            return Err(ObjectError::PackageConflict);
                        }
                    }
                    if home == Word::NIL {
                        put(
                            ctx,
                            *symbol,
                            ncl_object::layout::symbol_offset::PACKAGE,
                            *package,
                        )?;
                    }
                    Package::from_word(*package)
                        .insert_table_symbol(ctx, runtime, false, *name, *symbol)
                })
            })
        })
    }

    fn resolve_local_nickname(
        self,
        ctx: &ThreadContext,
        nickname: ncl_object::StringObject,
    ) -> Result<Option<Package>, ObjectError> {
        let wanted = nickname.as_word();
        let wanted_length = ncl_object::string_length(ctx, wanted)?;
        let mut entries = self.local_nicknames(ctx)?;
        while entries != Word::NIL {
            let entry = ncl_object::car(ctx, entries)?;
            let next = ncl_object::cdr(ctx, entries)?;
            let entry_name = ncl_object::car(ctx, entry)?;
            if ncl_object::string_length(ctx, entry_name)? == wanted_length
                && (0..wanted_length).all(|index| {
                    ncl_object::string_ref(ctx, entry_name, index)
                        == ncl_object::string_ref(ctx, wanted, index)
                })
            {
                let package = ncl_object::cdr(ctx, entry)?;
                return Package::try_from_word(ctx, package).map(Some);
            }
            entries = next;
        }
        Ok(None)
    }
}

use crate::PackageOperations;
use ncl_hash_tables::HashTable;
use ncl_object::package::{EXTERNAL, INTERNAL, NICKNAMES};
use ncl_object::widetag;
use ncl_object::{ObjectError, Package, Runtime, ThreadContext, Word, get, make_cons, put};

impl PackageOperations for Package {
    fn for_each_symbol<F>(self, ctx: &ThreadContext, mut visit: F) -> Result<(), ObjectError>
    where
        F: FnMut(Word),
    {
        for slot in std::iter::once(INTERNAL).chain(std::iter::once(EXTERNAL)) {
            HashTable::from_word(get(ctx, self.as_word(), widetag::PACKAGE, slot)?)
                .for_each_entry(ctx, |_, symbol| visit(symbol))?;
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
            Self::from_word(*package).ensure_unlocked(ctx)?;
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
            Self::from_word(*package).ensure_unlocked(ctx)?;
            let mut name = name;
            ncl_object::with_root(ctx, &mut name, |ctx, name| {
                let internal =
                    HashTable::from_word(get(ctx, *package, widetag::PACKAGE, INTERNAL)?);
                let mut symbol = if let Some(symbol) = internal.remove(ctx, runtime, *name)? {
                    symbol
                } else {
                    let external =
                        HashTable::from_word(get(ctx, *package, widetag::PACKAGE, EXTERNAL)?);
                    if external.get(ctx, *name)?.is_some() {
                        return Ok(true);
                    }
                    let Some((symbol, ncl_object::FindStatus::Inherited)) =
                        Self::from_word(*package).find_symbol(ctx, runtime, *name)?
                    else {
                        return Ok(false);
                    };
                    Self::from_word(*package).import(ctx, runtime, *name, symbol)?;
                    HashTable::from_word(get(ctx, *package, widetag::PACKAGE, INTERNAL)?)
                        .remove(ctx, runtime, *name)?
                        .ok_or(ObjectError::Layout)?
                };
                ncl_object::with_root(ctx, &mut symbol, |ctx, symbol| {
                    HashTable::from_word(get(ctx, *package, widetag::PACKAGE, EXTERNAL)?)
                        .insert(ctx, runtime, *name, *symbol)
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
            Self::from_word(*package).ensure_unlocked(ctx)?;
            let mut name = name;
            ncl_object::with_root(ctx, &mut name, |ctx, name| {
                let external =
                    HashTable::from_word(get(ctx, *package, widetag::PACKAGE, EXTERNAL)?);
                let Some(mut symbol) = external.remove(ctx, runtime, *name)? else {
                    return Ok(false);
                };
                ncl_object::with_root(ctx, &mut symbol, |ctx, symbol| {
                    HashTable::from_word(get(ctx, *package, widetag::PACKAGE, INTERNAL)?)
                        .insert(ctx, runtime, *name, *symbol)
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
            Self::from_word(*package).ensure_unlocked(ctx)?;
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
                        Self::from_word(*package).find_symbol(ctx, runtime, *name)?
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
                    HashTable::from_word(get(ctx, *package, widetag::PACKAGE, INTERNAL)?)
                        .insert(ctx, runtime, *name, *symbol)
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
                return Self::try_from_word(ctx, package).map(Some);
            }
            entries = next;
        }
        Ok(None)
    }
}

use super::{declaration_form, get_property, plist_property, set_plist_property, set_property};
use crate::form::{elements, symbol};
use ncl_object::{
    BuiltinArgs, ObjectError, ObjectRef, Package, Runtime, ThreadContext, Word, classify_object,
};

fn defsetf_documentation(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    mut name: Word,
) -> Result<Word, ObjectError> {
    ncl_object::with_root(ctx, &mut name, |ctx, name| {
        let mut property = symbol(ctx, runtime, "NCL::DEFINITION")?;
        ncl_object::with_root(ctx, &mut property, |ctx, property| {
            let definition = get_property(ctx, *name, *property)?;
            ncl_object::with_root(ctx, &mut definition.clone(), |ctx, definition| {
                if *definition == Word::NIL {
                    return Ok(Word::NIL);
                }
                let parts = elements(ctx, *definition)?;
                ncl_object::with_roots(ctx, &parts, |ctx, roots| {
                    let first = roots.first().ok_or(ObjectError::TypeError)?.get();
                    if first != Word::NIL
                        && matches!(
                            ncl_object::classify_object(ctx, first),
                            ObjectRef::Symbol(_)
                        )
                    {
                        return roots.get(1).map_or(Ok(Word::NIL), |doc| {
                            let word = doc.get();
                            if matches!(
                                ncl_object::classify_object(ctx, word),
                                ObjectRef::String(_)
                            ) {
                                Ok(word)
                            } else {
                                Ok(Word::NIL)
                            }
                        });
                    }
                    let mut leading = true;
                    for form in roots.iter().skip(2) {
                        let word = form.get();
                        if leading
                            && matches!(
                                ncl_object::classify_object(ctx, word),
                                ObjectRef::String(_)
                            )
                        {
                            return Ok(word);
                        }
                        if leading && declaration_form(ctx, runtime, word)? {
                            continue;
                        }
                        leading = false;
                    }
                    Ok(Word::NIL)
                })
            })
        })
    })
}

pub(super) fn documentation_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let name = args.required(0)?;
    let kind = args.required(1)?;
    if matches!(classify_object(ctx, name), ObjectRef::Package(_)) {
        if kind != Word::TRUE {
            return Err(ObjectError::TypeError);
        }
        return Package::from_word(name).documentation(ctx);
    }
    if !matches!(classify_object(ctx, name), ObjectRef::Symbol(_)) {
        return Err(ObjectError::TypeError);
    }
    ncl_object::with_roots(ctx, &[name, kind], |ctx, roots| {
        let mut documentation_key = symbol(ctx, runtime, "NCL::DOCUMENTATION")?;
        ncl_object::with_root(ctx, &mut documentation_key, |ctx, documentation_key| {
            let table = get_property(
                ctx,
                roots[0].get(), // check-added-lines: allow(index) roots has the two required arguments above.
                *documentation_key,
            )?;
            if let Some(documentation) = plist_property(
                ctx,
                table,
                roots[1].get(), // check-added-lines: allow(index) roots has the two required arguments above.
            )? {
                return Ok(documentation);
            }
            let mut setf = symbol(ctx, runtime, "SETF")?;
            ncl_object::with_root(ctx, &mut setf, |ctx, setf| {
                // check-added-lines: allow(index) roots has the two required arguments above.
                if roots[1].get() == *setf {
                    defsetf_documentation(
                        ctx,
                        runtime,
                        roots[0].get(), // check-added-lines: allow(index) roots has the two required arguments above.
                    )
                } else {
                    Ok(Word::NIL)
                }
            })
        })
    })
}

pub(super) fn set_documentation_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let name = args.required(0)?;
    let kind = args.required(1)?;
    let value = args.required(2)?;
    if !matches!(classify_object(ctx, value), ObjectRef::String(_)) {
        return Err(ObjectError::TypeError);
    }
    if matches!(classify_object(ctx, name), ObjectRef::Package(_)) {
        if kind != Word::TRUE {
            return Err(ObjectError::TypeError);
        }
        Package::from_word(name).set_documentation(ctx, value)?;
        return Ok(value);
    }
    if matches!(classify_object(ctx, name), ObjectRef::Symbol(_)) {
        return ncl_object::with_roots(ctx, &[name, kind, value], |ctx, roots| {
            let mut documentation_key = symbol(ctx, runtime, "NCL::DOCUMENTATION")?;
            ncl_object::with_root(ctx, &mut documentation_key, |ctx, documentation_key| {
                let name = roots[0].get(); // check-added-lines: allow(index) roots has the three required arguments above.
                let kind = roots[1].get(); // check-added-lines: allow(index) roots has the three required arguments above.
                let value = roots[2].get(); // check-added-lines: allow(index) roots has the three required arguments above.
                let table = get_property(ctx, name, *documentation_key)?;
                let table = set_plist_property(ctx, runtime, table, kind, value)?;
                set_property(ctx, runtime, name, *documentation_key, table)?;
                Ok(value)
            })
        });
    }
    Err(ObjectError::TypeError)
}

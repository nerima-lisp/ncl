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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form::list;
    use ncl_object::{BuiltinArgs, MultipleValues};

    fn fixture() -> Result<(Runtime, ThreadContext), ObjectError> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        Ok((runtime, ctx))
    }

    fn string(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        value: &str,
    ) -> Result<Word, ObjectError> {
        ncl_object::make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())
    }

    fn call_documentation(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        arguments: &[Word],
    ) -> Result<Word, ObjectError> {
        documentation_builtin(
            ctx,
            runtime,
            &BuiltinArgs::new(arguments),
            &mut MultipleValues::new(),
        )
    }

    fn set_documentation(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        arguments: &[Word],
    ) -> Result<Word, ObjectError> {
        set_documentation_builtin(
            ctx,
            runtime,
            &BuiltinArgs::new(arguments),
            &mut MultipleValues::new(),
        )
    }

    #[test]
    fn symbol_and_package_documentation_round_trip_with_type_checks() -> Result<(), ObjectError> {
        let (runtime, mut ctx) = fixture()?;
        let name = symbol(&mut ctx, &runtime, "SETF-DOC-SYMBOL")?;
        let kind = symbol(&mut ctx, &runtime, "FUNCTION")?;
        let doc = string(&mut ctx, &runtime, "function docs")?;
        assert_eq!(
            call_documentation(&mut ctx, &runtime, &[name, kind])?,
            Word::NIL
        );
        assert_eq!(
            set_documentation(&mut ctx, &runtime, &[name, kind, doc])?,
            doc
        );
        assert_eq!(call_documentation(&mut ctx, &runtime, &[name, kind])?, doc);

        let updated = string(&mut ctx, &runtime, "updated docs")?;
        set_documentation(&mut ctx, &runtime, &[name, kind, updated])?;
        assert_eq!(
            call_documentation(&mut ctx, &runtime, &[name, kind])?,
            updated
        );

        let documentation_key = symbol(&mut ctx, &runtime, "NCL::DOCUMENTATION")?;
        let malformed_table = ncl_object::make_cons(&mut ctx, &runtime, kind, Word::fixnum(7))?;
        super::super::set_property(&mut ctx, &runtime, name, documentation_key, malformed_table)?;
        assert!(matches!(
            set_documentation(&mut ctx, &runtime, &[name, kind, doc]),
            Err(ObjectError::TypeError)
        ));
        super::super::set_property(&mut ctx, &runtime, name, documentation_key, Word::fixnum(7))?;
        assert!(matches!(
            set_documentation(&mut ctx, &runtime, &[name, kind, doc]),
            Err(ObjectError::TypeError)
        ));

        let package_word = runtime.ensure_package(&mut ctx, "SETF-DOC-PACKAGE")?;
        let package_doc = string(&mut ctx, &runtime, "package docs")?;
        set_documentation(&mut ctx, &runtime, &[package_word, Word::TRUE, package_doc])?;
        assert_eq!(
            call_documentation(&mut ctx, &runtime, &[package_word, Word::TRUE])?,
            package_doc
        );
        assert!(matches!(
            call_documentation(&mut ctx, &runtime, &[package_word, kind]),
            Err(ObjectError::TypeError)
        ));
        assert!(matches!(
            set_documentation(&mut ctx, &runtime, &[package_word, kind, package_doc]),
            Err(ObjectError::TypeError)
        ));
        assert!(matches!(
            set_documentation(&mut ctx, &runtime, &[name, kind, Word::fixnum(1)]),
            Err(ObjectError::TypeError)
        ));
        assert!(matches!(
            call_documentation(&mut ctx, &runtime, &[Word::fixnum(1), kind]),
            Err(ObjectError::TypeError)
        ));
        assert!(matches!(
            set_documentation(&mut ctx, &runtime, &[Word::fixnum(1), kind, doc]),
            Err(ObjectError::TypeError)
        ));
        Ok(())
    }

    #[test]
    fn setf_documentation_reads_short_and_long_definition_forms() -> Result<(), ObjectError> {
        let (runtime, mut ctx) = fixture()?;
        let name = symbol(&mut ctx, &runtime, "SETF-DOC-FUNCTION")?;
        let definition_key = symbol(&mut ctx, &runtime, "NCL::DEFINITION")?;
        let setf_kind = symbol(&mut ctx, &runtime, "SETF")?;
        let setf_name = symbol(&mut ctx, &runtime, "SETF-DOC-FUNCTION")?;
        let no_doc_definition = list(&mut ctx, &runtime, &[setf_name])?;
        super::super::set_property(&mut ctx, &runtime, name, definition_key, no_doc_definition)?;
        assert_eq!(
            call_documentation(&mut ctx, &runtime, &[name, setf_kind])?,
            Word::NIL
        );
        let short_doc = string(&mut ctx, &runtime, "short definition docs")?;
        let short_definition = list(&mut ctx, &runtime, &[setf_name, short_doc])?;
        super::super::set_property(&mut ctx, &runtime, name, definition_key, short_definition)?;
        assert_eq!(
            call_documentation(&mut ctx, &runtime, &[name, setf_kind])?,
            short_doc
        );

        let variable = symbol(&mut ctx, &runtime, "X")?;
        let declare = symbol(&mut ctx, &runtime, "DECLARE")?;
        let special = symbol(&mut ctx, &runtime, "SPECIAL")?;
        let declaration_body = list(&mut ctx, &runtime, &[special, variable])?;
        let declaration = list(&mut ctx, &runtime, &[declare, declaration_body])?;
        let long_doc = string(&mut ctx, &runtime, "long definition docs")?;
        let long_definition = list(
            &mut ctx,
            &runtime,
            &[Word::NIL, Word::NIL, declaration, long_doc],
        )?;
        super::super::set_property(&mut ctx, &runtime, name, definition_key, long_definition)?;
        assert_eq!(
            call_documentation(&mut ctx, &runtime, &[name, setf_kind])?,
            long_doc
        );

        let long_definition_without_doc = list(
            &mut ctx,
            &runtime,
            &[Word::NIL, Word::NIL, declaration, declaration],
        )?;
        super::super::set_property(
            &mut ctx,
            &runtime,
            name,
            definition_key,
            long_definition_without_doc,
        )?;
        assert_eq!(
            call_documentation(&mut ctx, &runtime, &[name, setf_kind])?,
            Word::NIL
        );

        let non_declaration = symbol(&mut ctx, &runtime, "NOT-A-DECLARATION")?;
        let ignored_doc = string(&mut ctx, &runtime, "not leading")?;
        let trailing_definition = list(
            &mut ctx,
            &runtime,
            &[Word::NIL, Word::NIL, non_declaration, ignored_doc],
        )?;
        super::super::set_property(
            &mut ctx,
            &runtime,
            name,
            definition_key,
            trailing_definition,
        )?;
        assert_eq!(
            call_documentation(&mut ctx, &runtime, &[name, setf_kind])?,
            Word::NIL
        );
        Ok(())
    }
}

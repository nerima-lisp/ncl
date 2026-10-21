use ncl_object::{Handle, HandleVec, Local, ObjectError, Runtime, Scope, ThreadContext, Word};

use crate::form::elements;

pub(super) fn declaration_form(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    form: Word,
) -> Result<bool, ObjectError> {
    if !form.is_cons() {
        return Ok(false);
    }
    ncl_object::with_root(ctx, &mut form.clone(), |ctx, form| {
        let parts = elements(ctx, *form)?;
        let Some(operator) = parts.first().copied() else {
            return Ok(false);
        };
        ncl_object::with_root(ctx, &mut operator.clone(), |ctx, operator| {
            let expected = crate::form::symbol(ctx, runtime, "DECLARE")?;
            Ok(*operator == expected)
        })
    })
}

fn validate_declaration_semantics(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    declaration: Word,
) -> Result<(), ObjectError> {
    ncl_object::with_heap_rooted_slice(ctx, &[declaration], |ctx, declarations| {
        let parts = elements(
            ctx,
            declarations.first().ok_or(ObjectError::TypeError)?.get(),
        )?;
        ncl_object::with_heap_rooted_slice(ctx, &parts, |ctx, parts| {
            for specifier in parts.iter().skip(1) {
                let specifier_word = specifier.get();
                if !specifier_word.is_cons() {
                    continue;
                }
                let components = elements(ctx, specifier_word)?;
                ncl_object::with_heap_rooted_slice(ctx, &components, |ctx, components| {
                    let name = components.first().ok_or(ObjectError::TypeError)?.get();
                    let common_lisp = runtime.find_package(ctx, "COMMON-LISP");
                    if common_lisp == Some(ncl_object::symbol_package(ctx, name)?) {
                        let name = ncl_object::symbol_name(ctx, name)?;
                        let name = (0..ncl_object::string_length(ctx, name)?)
                            .map(|index| ncl_object::string_ref(ctx, name, index))
                            .collect::<Result<String, _>>()?;
                        if matches!(
                            name.as_str(),
                            "SPECIAL" | "TYPE" | "FTYPE" | "DYNAMIC-EXTENT"
                        ) {
                            return Err(ObjectError::TypeError);
                        }
                    }
                    Ok(())
                })?;
            }
            Ok(())
        })
    })
}

pub(super) fn defsetf_body_forms<'scope>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    definitions: &HandleVec<'scope, Word>,
) -> Result<HandleVec<'scope, Word>, ObjectError> {
    let mut body = Vec::<Handle<'scope, Word>>::new();
    let mut leading = true;
    let mut documentation = false;
    for form in definitions.as_slice().iter().skip(2) {
        let word = scope.get(*form).as_word();
        if leading
            && matches!(
                ncl_object::classify_object(scope.context(), word),
                ncl_object::ObjectRef::String(_)
            )
        {
            if documentation {
                return Err(ObjectError::TypeError);
            }
            documentation = true;
            continue;
        }
        if leading && word.is_cons() && declaration_form(scope.context_mut(), runtime, word)? {
            let declaration = scope.get(*form).as_word();
            validate_declaration_semantics(scope.context_mut(), runtime, declaration)?;
            continue;
        }
        leading = false;
        body.push(*form);
    }
    if body.is_empty() {
        return Err(ObjectError::TypeError);
    }
    let words = body
        .iter()
        .map(|form| Local::from_word(scope.get(*form).as_word()))
        .collect::<Vec<_>>();
    Ok(scope.root_many(&words))
}

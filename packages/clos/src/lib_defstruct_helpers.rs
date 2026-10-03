fn defstruct_slot_spec<'a>(
    scope: &mut ncl_object::Scope<'a>,
    fields: &ncl_object::HandleVec<'a, Word>,
) -> Result<
    (
        ncl_object::Handle<'a, Word>,
        ncl_object::Handle<'a, Word>,
        bool,
    ),
    ObjectError,
> {
    let slot = scope.root(ncl_object::Local::from_word(
        scope
            .get(*fields.as_slice().first().ok_or(ObjectError::TypeError)?)
            .as_word(),
    ));
    let initform = match fields.as_slice().get(1) {
        Some(handle) => scope.root(ncl_object::Local::from_word(scope.get(*handle).as_word())),
        None => scope.root(ncl_object::Local::from_word(Word::NIL)),
    };
    let mut read_only = false;
    let mut index = 2;
    while index + 1 < fields.len() {
        let key = *fields.as_slice().get(index).ok_or(ObjectError::TypeError)?;
        let value = *fields
            .as_slice()
            .get(index + 1)
            .ok_or(ObjectError::TypeError)?;
        let key_name = symbol_name_string(scope.context(), scope.get(key).as_word())?;
        if key_name == ":READ-ONLY" || key_name == "READ-ONLY" {
            read_only = !defstruct_is_nil(scope.context(), scope.get(value).as_word())?;
        }
        index += 2;
    }
    Ok((slot, initform, read_only))
}
fn defstruct_is_nil(ctx: &ThreadContext, value: Word) -> Result<bool, ObjectError> {
    if value == Word::NIL {
        return Ok(true);
    }
    if !matches!(
        ncl_object::classify_object(ctx, value),
        ObjectRef::Symbol(_)
    ) {
        return Ok(false);
    }
    let package = ncl_object::symbol_package(ctx, value)?;
    let package_name_word = ncl_object::Package::from_word(package).name(ctx)?;
    let package_name = (0..ncl_object::string_length(ctx, package_name_word)?)
        .map(|index| ncl_object::string_ref(ctx, package_name_word, index))
        .collect::<Result<String, _>>()?;
    Ok(package_name == COMMON_LISP && symbol_name_string(ctx, value)? == "NIL")
}

fn defstruct_name_or_nil(
    scope: &ncl_object::Scope<'_>,
    value: Word,
) -> Result<Option<String>, ObjectError> {
    if defstruct_is_nil(scope.context(), value)? {
        Ok(None)
    } else {
        symbol_name_string(scope.context(), value).map(Some)
    }
}

fn defstruct_boa_parameter(
    scope: &mut ncl_object::Scope<'_>,
    value: Word,
) -> Result<Option<Word>, ObjectError> {
    if !value.is_cons() {
        return Ok(Some(value));
    }
    let fields = scope.list_to_handle_vec(ncl_object::Local::from_word(value))?;
    let first = scope
        .get(*fields.as_slice().first().ok_or(ObjectError::TypeError)?)
        .as_word();
    if first.is_cons() {
        let key_fields = scope.list_to_handle_vec(ncl_object::Local::from_word(first))?;
        Ok(key_fields
            .as_slice()
            .get(1)
            .map(|handle| scope.get(*handle).as_word()))
    } else {
        Ok(Some(first))
    }
}

fn defstruct_boa_slot_parameters<'a>(
    scope: &mut ncl_object::Scope<'a>,
    lambda: Word,
) -> Result<Vec<(String, ncl_object::Handle<'a, Word>)>, ObjectError> {
    let fields = scope.list_to_handle_vec(ncl_object::Local::from_word(lambda))?;
    let mut parameters = Vec::new();
    let mut aux = false;
    for field in fields.iter().copied() {
        let value = scope.get(field).as_word();
        let name = if matches!(
            ncl_object::classify_object(scope.context(), value),
            ObjectRef::Symbol(_)
        ) {
            symbol_name_string(scope.context(), value)?
        } else {
            String::new()
        };
        if name.starts_with('&') {
            aux = name == "&AUX";
            continue;
        }
        if aux {
            continue;
        }
        let Some(parameter) = defstruct_boa_parameter(scope, value)? else {
            continue;
        };
        let parameter_name = symbol_name_string(scope.context(), parameter)?;
        parameters.push((
            parameter_name,
            scope.root(ncl_object::Local::from_word(parameter)),
        ));
    }
    Ok(parameters)
}

#[cfg(test)]
mod defstruct_helper_tests {
    use super::*;

    fn setup() -> (Runtime, ThreadContext) {
        let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("context registration: {error:?}"));
        (runtime, ctx)
    }

    fn intern(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
        let package = runtime
            .find_package(ctx, "COMMON-LISP-USER")
            .unwrap_or_else(|| panic!("COMMON-LISP-USER package"));
        Package::from_word(package)
            .intern(ctx, runtime, name)
            .unwrap_or_else(|error| panic!("intern: {error:?}"))
            .0
    }

    fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Word {
        let mut scope = ncl_object::Scope::new(ctx);
        let roots = scope.root_many(
            &values
                .iter()
                .copied()
                .map(ncl_object::Local::from_word)
                .collect::<Vec<_>>(),
        );
        let result = scope
            .make_list(runtime, &roots)
            .unwrap_or_else(|error| panic!("proper list: {error:?}"));
        scope.get(result).as_word()
    }

    #[test]
    fn slot_specs_and_nil_names_return_exact_values() -> Result<(), ObjectError> {
        let (runtime, mut ctx) = setup();
        let slot = intern(&runtime, &mut ctx, "VALUE");
        let read_only = intern(&runtime, &mut ctx, ":READ-ONLY");

        let mut scope = ncl_object::Scope::new(&mut ctx);
        let fields_word = list(
            scope.context_mut(),
            &runtime,
            &[slot, Word::fixnum(42), read_only, Word::TRUE],
        );
        let fields = scope.list_to_handle_vec(ncl_object::Local::from_word(fields_word))?;
        let (actual_slot, initform, is_read_only) = defstruct_slot_spec(&mut scope, &fields)?;
        assert_eq!(scope.get(actual_slot).as_word(), slot);
        assert_eq!(scope.get(initform).as_word(), Word::fixnum(42));
        assert!(is_read_only);

        let no_initform = list(scope.context_mut(), &runtime, &[slot]);
        let no_initform_fields =
            scope.list_to_handle_vec(ncl_object::Local::from_word(no_initform))?;
        let (_, initform, is_read_only) = defstruct_slot_spec(&mut scope, &no_initform_fields)?;
        assert_eq!(scope.get(initform).as_word(), Word::NIL);
        assert!(!is_read_only);

        let nil_read_only = list(
            scope.context_mut(),
            &runtime,
            &[slot, Word::NIL, read_only, Word::NIL],
        );
        let nil_read_only_fields =
            scope.list_to_handle_vec(ncl_object::Local::from_word(nil_read_only))?;
        let (_, _, is_read_only) = defstruct_slot_spec(&mut scope, &nil_read_only_fields)?;
        assert!(!is_read_only);

        let empty_fields = scope.list_to_handle_vec(ncl_object::Local::from_word(Word::NIL))?;
        assert_eq!(
            defstruct_slot_spec(&mut scope, &empty_fields),
            Err(ObjectError::TypeError)
        );
        assert!(defstruct_is_nil(scope.context(), Word::NIL)?);
        assert!(!defstruct_is_nil(scope.context(), Word::fixnum(7))?);
        assert!(!defstruct_is_nil(scope.context(), slot)?);
        assert_eq!(defstruct_name_or_nil(&scope, Word::NIL)?, None);
        assert_eq!(
            defstruct_name_or_nil(&scope, slot)?,
            Some(String::from("VALUE"))
        );
        Ok(())
    }

    #[test]
    fn boa_parameters_cover_plain_key_nested_and_malformed_values() -> Result<(), ObjectError> {
        let (runtime, mut ctx) = setup();
        let value = intern(&runtime, &mut ctx, "VALUE");
        let key = intern(&runtime, &mut ctx, ":VALUE");
        let mut scope = ncl_object::Scope::new(&mut ctx);

        assert_eq!(defstruct_boa_parameter(&mut scope, value)?, Some(value));

        let plain = list(scope.context_mut(), &runtime, &[value]);
        assert_eq!(defstruct_boa_parameter(&mut scope, plain)?, Some(value));

        let key_form = list(scope.context_mut(), &runtime, &[key, value]);
        let nested = list(scope.context_mut(), &runtime, &[key_form]);
        assert_eq!(defstruct_boa_parameter(&mut scope, nested)?, Some(value));

        let short_key_form = list(scope.context_mut(), &runtime, &[key]);
        let short_nested = list(scope.context_mut(), &runtime, &[short_key_form]);
        assert_eq!(defstruct_boa_parameter(&mut scope, short_nested)?, None);

        let dotted = ncl_object::make_cons(scope.context_mut(), &runtime, value, Word::fixnum(1))?;
        assert_eq!(
            defstruct_boa_parameter(&mut scope, dotted),
            Err(ObjectError::TypeError)
        );
        Ok(())
    }

    #[test]
    fn boa_slot_parameters_skip_non_symbols_and_aux_fields() -> Result<(), ObjectError> {
        let (runtime, mut ctx) = setup();
        let first = intern(&runtime, &mut ctx, "FIRST");
        let optional = intern(&runtime, &mut ctx, "&OPTIONAL");
        let aux = intern(&runtime, &mut ctx, "&AUX");
        let ignored = intern(&runtime, &mut ctx, "IGNORED");
        let second = intern(&runtime, &mut ctx, "SECOND");
        let mut scope = ncl_object::Scope::new(&mut ctx);
        let malformed = list(scope.context_mut(), &runtime, &[Word::fixnum(1)]);
        assert_eq!(
            defstruct_boa_slot_parameters(&mut scope, malformed),
            Err(ObjectError::TypeError)
        );
        let lambda = list(
            scope.context_mut(),
            &runtime,
            &[first, optional, second, aux, ignored],
        );

        let parameters = defstruct_boa_slot_parameters(&mut scope, lambda)?;
        assert_eq!(parameters.len(), 2);
        assert_eq!(parameters[0].0, "FIRST");
        assert_eq!(parameters[1].0, "SECOND");
        assert_eq!(scope.get(parameters[0].1).as_word(), first);
        assert_eq!(scope.get(parameters[1].1).as_word(), second);
        Ok(())
    }
}

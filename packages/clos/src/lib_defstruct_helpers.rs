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

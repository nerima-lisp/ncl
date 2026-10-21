use super::{MacroEnvironment, binding, sequence, wrap_let};
use crate::{SetfExpansion, fresh_symbol, list};
use ncl_object::{
    Handle, HandleVec, Local, ObjectError, ObjectRef, Runtime, Scope, ThreadContext, Word,
    classify_object,
};

#[path = "setf_defsetf_eval.rs"]
mod eval;
pub(super) use eval::eval_macro_form;

pub(super) fn defsetf_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    operator: Word,
    arguments: &[Word],
) -> Result<Option<SetfExpansion>, ObjectError> {
    let mut scope = Scope::new(ctx);
    let operator = scope.root(Local::<Word>::from_word(operator));
    let arguments = scope.root_many(
        &arguments
            .iter()
            .copied()
            .map(Local::<Word>::from_word)
            .collect::<Vec<_>>(),
    );
    let property = scope.intern(runtime, "NCL", "DEFINITION")?;
    let property_value = crate::definition_support::get_property(
        scope.context(),
        scope.get(operator).as_word(),
        scope.get(property).as_word(),
    )?;
    if property_value == Word::NIL {
        return Ok(None);
    }
    let property_value = scope.root(Local::<Word>::from_word(property_value));
    let definition_word = scope.get(property_value).as_word();
    let definitions = scope.list_to_handle_vec(Local::<Word>::from_word(definition_word))?;
    let first = definitions
        .as_slice()
        .first()
        .copied()
        .ok_or(ObjectError::TypeError)?;
    let first_word = scope.get(first).as_word();
    let is_short = first_word != Word::NIL
        && matches!(
            classify_object(scope.context(), first_word),
            ObjectRef::Symbol(_)
        );
    let expansion = if is_short {
        short_expansion(&mut scope, runtime, operator, &arguments, &definitions)?
    } else {
        long_expansion(&mut scope, runtime, operator, &arguments, &definitions)?
    };
    Ok(Some(expansion))
}

fn temporary_variables<'scope>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    count: usize,
) -> Result<HandleVec<'scope, Word>, ObjectError> {
    let mut variables = scope.root_many::<Word>(&[]);
    for _ in 0..count {
        let variable = fresh_symbol(scope.context_mut(), runtime)?;
        variables.push(scope, Local::<Word>::from_word(variable));
    }
    Ok(variables)
}

fn access_form<'scope>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    operator: Handle<'scope, Word>,
    temporaries: &HandleVec<'scope, Word>,
) -> Result<Handle<'scope, Word>, ObjectError> {
    let mut access = vec![scope.get(operator).as_word()];
    access.extend(
        temporaries
            .as_slice()
            .iter()
            .map(|temporary| scope.get(*temporary).as_word()),
    );
    let access = list(scope.context_mut(), runtime, &access)?;
    Ok(scope.root(Local::<Word>::from_word(access)))
}

fn setf_expansion<'scope>(
    scope: &Scope<'scope>,
    arguments: &HandleVec<'scope, Word>,
    temporaries: &HandleVec<'scope, Word>,
    store: Handle<'scope, Word>,
    store_form: Handle<'scope, Word>,
    access_form: Handle<'scope, Word>,
) -> SetfExpansion {
    SetfExpansion {
        temporary_variables: temporaries
            .as_slice()
            .iter()
            .map(|temporary| scope.get(*temporary).as_word())
            .collect(),
        value_forms: arguments
            .as_slice()
            .iter()
            .map(|argument| scope.get(*argument).as_word())
            .collect(),
        store_variables: vec![scope.get(store).as_word()],
        store_form: scope.get(store_form).as_word(),
        access_form: scope.get(access_form).as_word(),
    }
}

fn short_expansion<'scope>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    operator: Handle<'scope, Word>,
    arguments: &HandleVec<'scope, Word>,
    definitions: &HandleVec<'scope, Word>,
) -> Result<SetfExpansion, ObjectError> {
    if !(1..=2).contains(&definitions.len()) {
        return Err(ObjectError::TypeError);
    }
    if definitions.len() == 2 {
        let documentation = definitions
            .as_slice()
            .get(1)
            .copied()
            .ok_or(ObjectError::TypeError)?;
        if !matches!(
            classify_object(scope.context(), scope.get(documentation).as_word()),
            ObjectRef::String(_)
        ) {
            return Err(ObjectError::TypeError);
        }
    }
    let updater = *definitions
        .as_slice()
        .first()
        .ok_or(ObjectError::TypeError)?;
    ncl_object::symbol_name(scope.context(), scope.get(updater).as_word())?;
    let temporaries = temporary_variables(scope, runtime, arguments.len())?;
    let store = fresh_symbol(scope.context_mut(), runtime)?;
    let store = scope.root(Local::<Word>::from_word(store));
    let mut call = vec![scope.get(updater).as_word()];
    call.extend(
        temporaries
            .as_slice()
            .iter()
            .map(|temporary| scope.get(*temporary).as_word()),
    );
    call.push(scope.get(store).as_word());
    let call = list(scope.context_mut(), runtime, &call)?;
    let call = scope.root(Local::<Word>::from_word(call));
    let store_form = call;
    let access_form = access_form(scope, runtime, operator, &temporaries)?;
    Ok(setf_expansion(
        scope,
        arguments,
        &temporaries,
        store,
        store_form,
        access_form,
    ))
}

fn parameter_list<'scope>(
    scope: &mut Scope<'scope>,
    definitions: &HandleVec<'scope, Word>,
    index: usize,
) -> Result<HandleVec<'scope, Word>, ObjectError> {
    let definition = definitions
        .as_slice()
        .get(index)
        .ok_or(ObjectError::TypeError)?;
    let word = scope.get(*definition).as_word();
    scope.list_to_handle_vec(Local::<Word>::from_word(word))
}

fn validated_store_parameter<'scope>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    place_parameters: &HandleVec<'scope, Word>,
    store_parameters: &HandleVec<'scope, Word>,
    argument_count: usize,
) -> Result<Handle<'scope, Word>, ObjectError> {
    if place_parameters.len() != argument_count || store_parameters.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    for parameter in place_parameters.as_slice() {
        ncl_object::symbol_name(scope.context(), scope.get(*parameter).as_word())?;
        for keyword in [
            "&OPTIONAL",
            "&REST",
            "&KEY",
            "&ALLOW-OTHER-KEYS",
            "&ENVIRONMENT",
            "&AUX",
            "&BODY",
            "&WHOLE",
            "&MACRO",
        ] {
            let unsupported = scope.intern(runtime, "COMMON-LISP", keyword)?;
            if scope.get(unsupported).as_word() == scope.get(*parameter).as_word() {
                return Err(ObjectError::TypeError);
            }
        }
    }
    let store_parameter = *store_parameters
        .as_slice()
        .first()
        .ok_or(ObjectError::TypeError)?;
    ncl_object::symbol_name(scope.context(), scope.get(store_parameter).as_word())?;
    for keyword in [
        "&OPTIONAL",
        "&REST",
        "&KEY",
        "&ALLOW-OTHER-KEYS",
        "&ENVIRONMENT",
        "&AUX",
        "&BODY",
        "&WHOLE",
        "&MACRO",
    ] {
        let unsupported = scope.intern(runtime, "COMMON-LISP", keyword)?;
        if scope.get(unsupported).as_word() == scope.get(store_parameter).as_word() {
            return Err(ObjectError::TypeError);
        }
    }
    Ok(store_parameter)
}

fn long_bindings<'scope>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    place_parameters: &HandleVec<'scope, Word>,
    temporaries: &HandleVec<'scope, Word>,
    store_parameter: Handle<'scope, Word>,
    store: Handle<'scope, Word>,
) -> Result<HandleVec<'scope, Word>, ObjectError> {
    let mut bindings = scope.root_many::<Word>(&[]);
    for (parameter, temporary) in place_parameters
        .as_slice()
        .iter()
        .zip(temporaries.as_slice())
    {
        let parameter_word = scope.get(*parameter).as_word();
        let temporary_word = scope.get(*temporary).as_word();
        let binding = binding(scope.context_mut(), runtime, parameter_word, temporary_word)?;
        bindings.push(scope, Local::<Word>::from_word(binding));
    }
    let parameter_word = scope.get(store_parameter).as_word();
    let store_word = scope.get(store).as_word();
    let binding = binding(scope.context_mut(), runtime, parameter_word, store_word)?;
    bindings.push(scope, Local::<Word>::from_word(binding));
    Ok(bindings)
}

pub(super) fn special_form(
    scope: &mut Scope<'_>,
    runtime: &Runtime,
    operator: Word,
    name: &str,
) -> Result<bool, ObjectError> {
    let operator = scope.root(Local::<Word>::from_word(operator));
    let expected = scope.intern(runtime, "COMMON-LISP", name)?;
    Ok(scope.get(expected).as_word() == scope.get(operator).as_word())
}

pub(super) fn quasiquote_marker<'scope>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    form: Handle<'scope, Word>,
    name: &str,
) -> Result<Option<Handle<'scope, Word>>, ObjectError> {
    let word = scope.get(form).as_word();
    if !word.is_cons() {
        return Ok(None);
    }
    let parts = scope.list_to_handle_vec(Local::<Word>::from_word(word))?;
    if parts.len() != 2 {
        return Ok(None);
    }
    let operator = *parts.as_slice().first().ok_or(ObjectError::TypeError)?;
    let expected = scope.intern(runtime, "COMMON-LISP", name)?;
    if scope.get(operator).as_word() == scope.get(expected).as_word() {
        Ok(parts.as_slice().get(1).copied())
    } else {
        Ok(None)
    }
}

pub(super) fn marker_form<'scope>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    name: &str,
    argument: Handle<'scope, Word>,
) -> Result<Handle<'scope, Word>, ObjectError> {
    let marker = scope.intern(runtime, "COMMON-LISP", name)?;
    let values = [scope.get(marker).as_word(), scope.get(argument).as_word()];
    let form = list(scope.context_mut(), runtime, &values)?;
    Ok(scope.root(Local::<Word>::from_word(form)))
}

pub(super) fn quasiquote_vector<'scope>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    vector: Handle<'scope, Word>,
    environment: &MacroEnvironment<'scope>,
    depth: u32,
) -> Result<Handle<'scope, Word>, ObjectError> {
    let word = scope.get(vector).as_word();
    let length = ncl_object::simple_vector_length(scope.context(), word)?;
    let mut elements = Vec::with_capacity(length);
    for index in 0..length {
        elements.push(ncl_object::simple_vector_ref(scope.context(), word, index)?);
    }
    let elements = scope.root_many(
        &elements
            .into_iter()
            .map(Local::<Word>::from_word)
            .collect::<Vec<_>>(),
    );
    let words = elements
        .as_slice()
        .iter()
        .map(|element| scope.get(*element).as_word())
        .collect::<Vec<_>>();
    let list = list(scope.context_mut(), runtime, &words)?;
    let list = scope.root(Local::<Word>::from_word(list));
    let expanded = super::eval_quasiquote(scope, runtime, list, environment, depth)?;
    let expanded_word = scope.get(expanded).as_word();
    let elements = scope.list_to_handle_vec(Local::<Word>::from_word(expanded_word))?;
    scope.make_simple_vector(runtime, &elements)
}

fn long_store_form<'scope>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    bindings: &HandleVec<'scope, Word>,
    store: Handle<'scope, Word>,
    expansion: Handle<'scope, Word>,
) -> Result<Handle<'scope, Word>, ObjectError> {
    let body = [scope.get(expansion).as_word(), scope.get(store).as_word()];
    let body = sequence(scope.context_mut(), runtime, &body)?;
    let body = scope.root(Local::<Word>::from_word(body));
    let binding_words = bindings
        .as_slice()
        .iter()
        .map(|item| scope.get(*item).as_word())
        .collect::<Vec<_>>();
    let body_word = scope.get(body).as_word();
    let store_form = wrap_let(
        scope.context_mut(),
        runtime,
        "LET",
        &binding_words,
        body_word,
    )?;
    Ok(scope.root(Local::<Word>::from_word(store_form)))
}

fn long_expansion<'scope>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    operator: Handle<'scope, Word>,
    arguments: &HandleVec<'scope, Word>,
    definitions: &HandleVec<'scope, Word>,
) -> Result<SetfExpansion, ObjectError> {
    let place_parameters = parameter_list(scope, definitions, 0)?;
    let store_parameters = parameter_list(scope, definitions, 1)?;
    let store_parameter = validated_store_parameter(
        scope,
        runtime,
        &place_parameters,
        &store_parameters,
        arguments.len(),
    )?;
    let temporaries = temporary_variables(scope, runtime, arguments.len())?;
    let store = fresh_symbol(scope.context_mut(), runtime)?;
    let store = scope.root(Local::<Word>::from_word(store));
    let bindings = long_bindings(
        scope,
        runtime,
        &place_parameters,
        &temporaries,
        store_parameter,
        store,
    )?;
    let body = crate::definition_support::defsetf_body_forms(scope, runtime, definitions)?;
    let mut environment = super::MacroEnvironment::new();
    for (parameter, temporary) in place_parameters
        .as_slice()
        .iter()
        .copied()
        .zip(temporaries.as_slice().iter().copied())
    {
        environment.push((parameter, temporary));
    }
    environment.push((store_parameter, store));
    let mut expansion = scope.root(Local::<Word>::from_word(Word::NIL));
    for form in body.as_slice() {
        expansion = eval_macro_form(scope, runtime, *form, &environment)?;
    }
    let store_form = long_store_form(scope, runtime, &bindings, store, expansion)?;
    let access_form = access_form(scope, runtime, operator, &temporaries)?;
    Ok(setf_expansion(
        scope,
        arguments,
        &temporaries,
        store,
        store_form,
        access_form,
    ))
}

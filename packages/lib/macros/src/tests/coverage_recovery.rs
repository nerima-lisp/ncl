use super::*;

fn fixture() -> Result<(Runtime, ThreadContext), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    Ok((runtime, ctx))
}

fn named(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Result<Word, ObjectError> {
    symbol(ctx, runtime, name)
}

fn list_of(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Word, ObjectError> {
    list(ctx, runtime, values)
}

fn call_macro(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    form: Word,
) -> Result<Word, ObjectError> {
    let callback = callback_for(name).ok_or(ObjectError::UndefinedFunction)?;
    callback(
        ctx,
        runtime,
        &ncl_object::BuiltinArgs::new(&[form]),
        &mut ncl_object::MultipleValues::new(),
    )
}

#[test]
fn control_macro_expanders_cover_handler_restart_and_multiple_value_forms()
-> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let body = named(&mut ctx, &runtime, "BODY")?;
    let handler = named(&mut ctx, &runtime, "HANDLER")?;
    let condition = named(&mut ctx, &runtime, "CONDITION")?;
    let t = named(&mut ctx, &runtime, "T")?;
    let handler_bind_name = named(&mut ctx, &runtime, "HANDLER-BIND")?;
    let handler_case_name = named(&mut ctx, &runtime, "HANDLER-CASE")?;
    let ignore_errors_name = named(&mut ctx, &runtime, "IGNORE-ERRORS")?;
    let restart_bind_name = named(&mut ctx, &runtime, "RESTART-BIND")?;
    let restart_case_name = named(&mut ctx, &runtime, "RESTART-CASE")?;
    let simple_restart_name = named(&mut ctx, &runtime, "WITH-SIMPLE-RESTART")?;
    let multiple_bind_name = named(&mut ctx, &runtime, "MULTIPLE-VALUE-BIND")?;
    let multiple_list_name = named(&mut ctx, &runtime, "MULTIPLE-VALUE-LIST")?;
    let handler_clause = list_of(&mut ctx, &runtime, &[t, handler])?;
    let handler_clauses = list_of(&mut ctx, &runtime, &[handler_clause])?;
    let handler_form = list_of(
        &mut ctx,
        &runtime,
        &[handler_bind_name, handler_clauses, body],
    )?;
    let expanded = call_macro(&runtime, &mut ctx, "HANDLER-BIND", handler_form)?;
    assert_eq!(
        elements(&mut ctx, expanded)?[0],
        named(&mut ctx, &runtime, "LET")?
    );

    let variable = named(&mut ctx, &runtime, "C")?;
    let variables = list_of(&mut ctx, &runtime, &[variable])?;
    let error_clause = list_of(&mut ctx, &runtime, &[condition, variables, body])?;
    let handler_case = list_of(&mut ctx, &runtime, &[handler_case_name, body, error_clause])?;
    let expanded = call_macro(&runtime, &mut ctx, "HANDLER-CASE", handler_case)?;
    assert_eq!(
        elements(&mut ctx, expanded)?[0],
        named(&mut ctx, &runtime, "BLOCK")?
    );

    let ignore_errors = list_of(&mut ctx, &runtime, &[ignore_errors_name, body])?;
    let expanded = call_macro(&runtime, &mut ctx, "IGNORE-ERRORS", ignore_errors)?;
    assert_eq!(
        elements(&mut ctx, expanded)?[0],
        named(&mut ctx, &runtime, "BLOCK")?
    );

    let restart_clause = list_of(&mut ctx, &runtime, &[handler, handler])?;
    let restart_clauses = list_of(&mut ctx, &runtime, &[restart_clause])?;
    let restart_bind = list_of(
        &mut ctx,
        &runtime,
        &[restart_bind_name, restart_clauses, body],
    )?;
    let expanded = call_macro(&runtime, &mut ctx, "RESTART-BIND", restart_bind)?;
    assert_eq!(
        elements(&mut ctx, expanded)?[0],
        named(&mut ctx, &runtime, "LET")?
    );

    let restart_case_clause = list_of(&mut ctx, &runtime, &[handler, variables, body])?;
    let restart_case = list_of(
        &mut ctx,
        &runtime,
        &[restart_case_name, body, restart_case_clause],
    )?;
    let expanded = call_macro(&runtime, &mut ctx, "RESTART-CASE", restart_case)?;
    assert_eq!(
        elements(&mut ctx, expanded)?[0],
        named(&mut ctx, &runtime, "BLOCK")?
    );

    let simple_heading = list_of(&mut ctx, &runtime, &[handler, body])?;
    let simple = list_of(
        &mut ctx,
        &runtime,
        &[simple_restart_name, simple_heading, body],
    )?;
    let expanded = call_macro(&runtime, &mut ctx, "WITH-SIMPLE-RESTART", simple)?;
    assert_eq!(
        elements(&mut ctx, expanded)?[0],
        named(&mut ctx, &runtime, "BLOCK")?
    );

    let a = named(&mut ctx, &runtime, "A")?;
    let values = list_of(&mut ctx, &runtime, &[a])?;
    let multiple_bind = list_of(
        &mut ctx,
        &runtime,
        &[multiple_bind_name, values, body, body],
    )?;
    let expanded = call_macro(&runtime, &mut ctx, "MULTIPLE-VALUE-BIND", multiple_bind)?;
    assert_eq!(
        elements(&mut ctx, expanded)?[0],
        named(&mut ctx, &runtime, "MULTIPLE-VALUE-CALL")?
    );

    let multiple_list = list_of(&mut ctx, &runtime, &[multiple_list_name, body])?;
    let expanded = call_macro(&runtime, &mut ctx, "MULTIPLE-VALUE-LIST", multiple_list)?;
    assert_eq!(
        elements(&mut ctx, expanded)?[0],
        named(&mut ctx, &runtime, "MULTIPLE-VALUE-CALL")?
    );
    Ok(())
}

#[test]
fn control_macro_expanders_cover_typecase_defaults_and_malformed_inputs() -> Result<(), ObjectError>
{
    let (runtime, mut ctx) = fixture()?;
    let body = named(&mut ctx, &runtime, "BODY")?;
    let value = named(&mut ctx, &runtime, "VALUE")?;
    let integer = named(&mut ctx, &runtime, "INTEGER")?;
    let typecase_name = named(&mut ctx, &runtime, "TYPECASE")?;
    let case_name = named(&mut ctx, &runtime, "CASE")?;
    let integer_clause = list_of(&mut ctx, &runtime, &[integer, body])?;
    let otherwise = named(&mut ctx, &runtime, "OTHERWISE")?;
    let otherwise_clause = list_of(&mut ctx, &runtime, &[otherwise, body])?;
    let clauses = list_of(&mut ctx, &runtime, &[integer_clause, otherwise_clause])?;
    let typecase = list_of(&mut ctx, &runtime, &[typecase_name, value, integer_clause])?;
    let expanded = call_macro(&runtime, &mut ctx, "TYPECASE", typecase)?;
    assert_eq!(
        elements(&mut ctx, expanded)?[0],
        named(&mut ctx, &runtime, "LET")?
    );
    let typecase_default = list_of(
        &mut ctx,
        &runtime,
        &[typecase_name, value, integer_clause, otherwise_clause],
    )?;
    assert!(call_macro(&runtime, &mut ctx, "TYPECASE", typecase_default).is_ok());
    let case = list_of(&mut ctx, &runtime, &[case_name, value, clauses])?;
    assert!(call_macro(&runtime, &mut ctx, "CASE", case).is_ok());
    let malformed = list_of(&mut ctx, &runtime, &[typecase_name])?;
    assert_eq!(
        call_macro(&runtime, &mut ctx, "TYPECASE", malformed),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn define_condition_expands_slots_accessors_and_report_option() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let name = symbol(&mut ctx, &runtime, "MY-CONDITION")?;
    let parent = symbol(&mut ctx, &runtime, "CONDITION")?;
    let slot = symbol(&mut ctx, &runtime, "MESSAGE")?;
    let initarg = symbol(&mut ctx, &runtime, "INITARG")?;
    let initform = symbol(&mut ctx, &runtime, "INITFORM")?;
    let reader = symbol(&mut ctx, &runtime, "READER")?;
    let accessor = symbol(&mut ctx, &runtime, "ACCESSOR")?;
    let initarg_name = symbol(&mut ctx, &runtime, "MESSAGE")?;
    let default = symbol(&mut ctx, &runtime, "DEFAULT-MESSAGE")?;
    let reader_name = symbol(&mut ctx, &runtime, "MY-MESSAGE")?;
    let accessor_name = symbol(&mut ctx, &runtime, "MY-MESSAGE-SET")?;
    let slot_spec = list(
        &mut ctx,
        &runtime,
        &[
            slot,
            initarg,
            initarg_name,
            initform,
            default,
            reader,
            reader_name,
            accessor,
            accessor_name,
        ],
    )?;
    let slots = list(&mut ctx, &runtime, &[slot_spec])?;
    let parents = list(&mut ctx, &runtime, &[parent])?;
    let report = symbol(&mut ctx, &runtime, "REPORT-FUNCTION")?;
    let report_key = symbol(&mut ctx, &runtime, "REPORT")?;
    let report_option = list(&mut ctx, &runtime, &[report_key, report])?;
    let define_condition = symbol(&mut ctx, &runtime, "DEFINE-CONDITION")?;
    let form = list(
        &mut ctx,
        &runtime,
        &[define_condition, name, parents, slots, report_option],
    )?;
    let expanded = call_macro(&runtime, &mut ctx, "DEFINE-CONDITION", form)?;
    let parts = elements(&mut ctx, expanded)?;
    assert_eq!(parts[0], symbol(&mut ctx, &runtime, "PROGN")?);
    assert!(parts.len() >= 3);
    let define = elements(&mut ctx, parts[1])?;
    assert_eq!(
        define[0],
        symbol(&mut ctx, &runtime, "NCL-EXT::DEFINE-CONDITION-CLASS")?
    );
    let defun = symbol(&mut ctx, &runtime, "DEFUN")?;
    assert!(
        parts[2..]
            .iter()
            .map(|part| elements(&mut ctx, *part))
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .any(|part| part.first() == Some(&defun))
    );
    Ok(())
}

#[test]
fn define_condition_rejects_missing_arguments_and_malformed_slot_lists() -> Result<(), ObjectError>
{
    let (runtime, mut ctx) = fixture()?;
    let operator = symbol(&mut ctx, &runtime, "DEFINE-CONDITION")?;
    let name = symbol(&mut ctx, &runtime, "C")?;
    let bad = list(&mut ctx, &runtime, &[operator, name])?;
    assert_eq!(
        call_macro(&runtime, &mut ctx, "DEFINE-CONDITION", bad),
        Err(ObjectError::TypeError)
    );
    let condition = symbol(&mut ctx, &runtime, "CONDITION")?;
    let parents = list(&mut ctx, &runtime, &[condition])?;
    let malformed_slots = symbol(&mut ctx, &runtime, "NOT-A-LIST")?;
    let form = list(
        &mut ctx,
        &runtime,
        &[operator, name, parents, malformed_slots],
    )?;
    assert_eq!(
        call_macro(&runtime, &mut ctx, "DEFINE-CONDITION", form),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn destructuring_key_patterns_expand_defaults_supplied_and_explicit_keywords()
-> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let op = symbol(&mut ctx, &runtime, "DESTRUCTURING-BIND")?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;
    let name = symbol(&mut ctx, &runtime, "NAME")?;
    let default = symbol(&mut ctx, &runtime, "DEFAULT")?;
    let supplied = symbol(&mut ctx, &runtime, "SUPPLIED")?;
    let key = symbol(&mut ctx, &runtime, "&KEY")?;
    let explicit = symbol(&mut ctx, &runtime, "EXPLICIT")?;
    let explicit_name = symbol(&mut ctx, &runtime, "EXPLICIT-NAME")?;
    let explicit_pair = list(&mut ctx, &runtime, &[explicit, explicit_name])?;
    let explicit_spec = list(&mut ctx, &runtime, &[explicit_pair, default])?;
    let key_spec = list(&mut ctx, &runtime, &[name, default, supplied])?;
    let pattern = list(&mut ctx, &runtime, &[key, key_spec, explicit_spec])?;
    let form = list(&mut ctx, &runtime, &[op, pattern, value, name])?;
    let expanded = call_macro(&runtime, &mut ctx, "DESTRUCTURING-BIND", form)?;
    let parts = elements(&mut ctx, expanded)?;
    assert_eq!(parts[0], symbol(&mut ctx, &runtime, "LET*")?);
    let bindings = elements(&mut ctx, parts[1])?;
    assert!(bindings.len() >= 6);
    let key_binding = bindings
        .iter()
        .map(|binding| elements(&mut ctx, *binding))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .find(|binding| binding.first() == Some(&name))
        .expect("bare key name binding");
    let key_value = elements(&mut ctx, key_binding[1])?;
    assert_eq!(key_value[0], symbol(&mut ctx, &runtime, "IF")?);
    assert!(key_value.len() >= 3);
    assert!(key_value.contains(&default));
    let supplied_binding = bindings
        .iter()
        .map(|binding| elements(&mut ctx, *binding))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .find(|binding| binding.first() == Some(&supplied))
        .expect("key supplied-p binding");
    assert_eq!(
        elements(&mut ctx, supplied_binding[1])?[0],
        symbol(&mut ctx, &runtime, "IF")?
    );
    let explicit_binding = bindings
        .iter()
        .map(|binding| elements(&mut ctx, *binding))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .find(|binding| binding.first() == Some(&explicit_name))
        .expect("explicit keyword name binding");
    assert!(elements(&mut ctx, explicit_binding[1])?.len() >= 3);
    Ok(())
}

#[test]
fn destructuring_optional_rest_whole_and_environment_expand_to_checked_bindings()
-> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let op = symbol(&mut ctx, &runtime, "DESTRUCTURING-BIND")?;
    let whole = symbol(&mut ctx, &runtime, "WHOLE")?;
    let environment = symbol(&mut ctx, &runtime, "ENVIRONMENT")?;
    let a = symbol(&mut ctx, &runtime, "A")?;
    let b = symbol(&mut ctx, &runtime, "B")?;
    let fallback = symbol(&mut ctx, &runtime, "FALLBACK")?;
    let supplied = symbol(&mut ctx, &runtime, "B-SUPPLIED")?;
    let rest = symbol(&mut ctx, &runtime, "REST")?;
    let whole_marker = symbol(&mut ctx, &runtime, "&WHOLE")?;
    let environment_marker = symbol(&mut ctx, &runtime, "&ENVIRONMENT")?;
    let optional_marker = symbol(&mut ctx, &runtime, "&OPTIONAL")?;
    let rest_marker = symbol(&mut ctx, &runtime, "&REST")?;
    let optional_spec = list(&mut ctx, &runtime, &[b, fallback, supplied])?;
    let pattern = list(
        &mut ctx,
        &runtime,
        &[
            whole_marker,
            whole,
            environment_marker,
            environment,
            a,
            optional_marker,
            optional_spec,
            rest_marker,
            rest,
        ],
    )?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;
    let form = list(&mut ctx, &runtime, &[op, pattern, value, a])?;
    let expanded = call_macro(&runtime, &mut ctx, "DESTRUCTURING-BIND", form)?;
    let parts = elements(&mut ctx, expanded)?;
    assert_eq!(parts[0], symbol(&mut ctx, &runtime, "LET*")?);
    let bindings = elements(&mut ctx, parts[1])?;
    let as_forms = bindings
        .iter()
        .map(|binding| elements(&mut ctx, *binding))
        .collect::<Result<Vec<_>, _>>()?;
    let whole_binding = as_forms
        .iter()
        .find(|binding| binding.first() == Some(&whole))
        .expect("&whole binding");
    assert_eq!(whole_binding[1], as_forms[0][0]);
    let environment_binding = as_forms
        .iter()
        .find(|binding| binding.first() == Some(&environment))
        .expect("&environment binding");
    assert_eq!(environment_binding[1], Word::NIL);
    let optional_binding = as_forms
        .iter()
        .find(|binding| binding.first() == Some(&b))
        .expect("&optional binding");
    let optional_value = elements(&mut ctx, optional_binding[1])?;
    assert_eq!(optional_value[0], symbol(&mut ctx, &runtime, "IF")?);
    assert!(optional_value.contains(&fallback));
    let supplied_binding = as_forms
        .iter()
        .find(|binding| binding.first() == Some(&supplied))
        .expect("optional supplied-p binding");
    assert_eq!(
        elements(&mut ctx, supplied_binding[1])?[0],
        symbol(&mut ctx, &runtime, "IF")?
    );
    let rest_binding = as_forms
        .iter()
        .find(|binding| binding.first() == Some(&rest))
        .expect("&rest binding");
    assert_ne!(rest_binding[1], Word::NIL);
    Ok(())
}

#[test]
fn destructuring_rejects_malformed_rest_key_and_whole_tails() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let op = symbol(&mut ctx, &runtime, "DESTRUCTURING-BIND")?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;
    let rest = symbol(&mut ctx, &runtime, "REST")?;
    let trailing = symbol(&mut ctx, &runtime, "TRAILING")?;
    let rest_marker = symbol(&mut ctx, &runtime, "&REST")?;
    let rest_pattern = list(&mut ctx, &runtime, &[rest_marker, rest, trailing])?;
    let rest_form = list(&mut ctx, &runtime, &[op, rest_pattern, value])?;
    assert_eq!(
        call_macro(&runtime, &mut ctx, "DESTRUCTURING-BIND", rest_form),
        Err(ObjectError::TypeError)
    );

    let key_marker = symbol(&mut ctx, &runtime, "&KEY")?;
    let dotted_key_pattern = ncl_object::make_cons(&mut ctx, &runtime, key_marker, rest)?;
    let key_form = list(&mut ctx, &runtime, &[op, dotted_key_pattern, value])?;
    assert_eq!(
        call_macro(&runtime, &mut ctx, "DESTRUCTURING-BIND", key_form),
        Err(ObjectError::TypeError)
    );

    let whole_marker = symbol(&mut ctx, &runtime, "&WHOLE")?;
    let missing_whole = list(&mut ctx, &runtime, &[whole_marker])?;
    let whole_form = list(&mut ctx, &runtime, &[op, missing_whole, value])?;
    assert_eq!(
        call_macro(&runtime, &mut ctx, "DESTRUCTURING-BIND", whole_form),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn destructuring_key_and_marker_errors_are_reported() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let op = symbol(&mut ctx, &runtime, "DESTRUCTURING-BIND")?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;
    for marker in ["&AUX", "&ALLOW-OTHER-KEYS"] {
        let marker_symbol = symbol(&mut ctx, &runtime, marker)?;
        let pattern = list(&mut ctx, &runtime, &[marker_symbol])?;
        let form = list(&mut ctx, &runtime, &[op, pattern, value])?;
        assert_eq!(
            call_macro(&runtime, &mut ctx, "DESTRUCTURING-BIND", form),
            Err(ObjectError::TypeError)
        );
    }
    Ok(())
}

#[test]
fn destructuring_optional_rest_whole_and_environment_bindings_expand_distinctly()
-> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let op = symbol(&mut ctx, &runtime, "DESTRUCTURING-BIND")?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;
    let whole = symbol(&mut ctx, &runtime, "WHOLE")?;
    let environment = symbol(&mut ctx, &runtime, "ENVIRONMENT")?;
    let optional = symbol(&mut ctx, &runtime, "OPTIONAL")?;
    let default = symbol(&mut ctx, &runtime, "DEFAULT")?;
    let supplied = symbol(&mut ctx, &runtime, "SUPPLIED")?;
    let rest = symbol(&mut ctx, &runtime, "REST")?;
    let whole_marker = symbol(&mut ctx, &runtime, "&WHOLE")?;
    let environment_marker = symbol(&mut ctx, &runtime, "&ENVIRONMENT")?;
    let optional_marker = symbol(&mut ctx, &runtime, "&OPTIONAL")?;
    let rest_marker = symbol(&mut ctx, &runtime, "&REST")?;
    let if_symbol = symbol(&mut ctx, &runtime, "IF")?;
    let optional_spec = list(&mut ctx, &runtime, &[optional, default, supplied])?;
    let pattern = list(
        &mut ctx,
        &runtime,
        &[
            whole_marker,
            whole,
            environment_marker,
            environment,
            optional_marker,
            optional_spec,
            rest_marker,
            rest,
        ],
    )?;
    let body = symbol(&mut ctx, &runtime, "BODY")?;
    let form = list(&mut ctx, &runtime, &[op, pattern, value, body])?;
    let expanded = call_macro(&runtime, &mut ctx, "DESTRUCTURING-BIND", form)?;
    let parts = elements(&mut ctx, expanded)?;
    assert_eq!(parts[0], symbol(&mut ctx, &runtime, "LET*")?);
    let bindings = elements(&mut ctx, parts[1])?;
    let source_binding = elements(&mut ctx, bindings[0])?;
    assert_eq!(source_binding[1], value);
    let source = source_binding[0];
    assert_eq!(elements(&mut ctx, bindings[1])?, vec![whole, source]);
    assert_eq!(
        elements(&mut ctx, bindings[2])?,
        vec![environment, Word::NIL]
    );

    let optional_binding = elements(&mut ctx, bindings[3])?;
    assert_eq!(optional_binding[0], optional);
    assert_eq!(elements(&mut ctx, optional_binding[1])?[0], if_symbol);
    let supplied_binding = elements(&mut ctx, bindings[4])?;
    assert_eq!(supplied_binding[0], supplied);
    assert_eq!(elements(&mut ctx, supplied_binding[1])?[0], if_symbol);
    assert_eq!(elements(&mut ctx, bindings[6])?, vec![rest, source]);
    assert_eq!(parts[2], body);
    Ok(())
}

#[test]
fn destructuring_key_supplied_p_and_custom_keyword_expand_to_lookup_guards()
-> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let op = symbol(&mut ctx, &runtime, "DESTRUCTURING-BIND")?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;
    let name = symbol(&mut ctx, &runtime, "NAME")?;
    let default = symbol(&mut ctx, &runtime, "DEFAULT")?;
    let supplied = symbol(&mut ctx, &runtime, "SUPPLIED")?;
    let keyword = symbol(&mut ctx, &runtime, ":CUSTOM")?;
    let key_marker = symbol(&mut ctx, &runtime, "&KEY")?;
    let if_symbol = symbol(&mut ctx, &runtime, "IF")?;
    let pair = list(&mut ctx, &runtime, &[keyword, name])?;
    let spec = list(&mut ctx, &runtime, &[pair, default, supplied])?;
    let pattern = list(&mut ctx, &runtime, &[key_marker, spec])?;
    let form = list(&mut ctx, &runtime, &[op, pattern, value, name])?;
    let expanded = call_macro(&runtime, &mut ctx, "DESTRUCTURING-BIND", form)?;
    let parts = elements(&mut ctx, expanded)?;
    let bindings = elements(&mut ctx, parts[1])?;
    let name_binding = bindings
        .iter()
        .map(|binding| elements(&mut ctx, *binding))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .find(|binding| binding.first() == Some(&name))
        .ok_or(ObjectError::TypeError)?;
    assert_eq!(elements(&mut ctx, name_binding[1])?[0], if_symbol);
    let supplied_binding = bindings
        .iter()
        .map(|binding| elements(&mut ctx, *binding))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .find(|binding| binding.first() == Some(&supplied))
        .ok_or(ObjectError::TypeError)?;
    assert_eq!(elements(&mut ctx, supplied_binding[1])?[0], if_symbol);
    Ok(())
}

#[test]
fn destructuring_rejects_incomplete_rest_and_key_specs() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let op = symbol(&mut ctx, &runtime, "DESTRUCTURING-BIND")?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;
    let rest_marker = symbol(&mut ctx, &runtime, "&REST")?;
    let key_marker = symbol(&mut ctx, &runtime, "&KEY")?;
    let missing_rest = list(&mut ctx, &runtime, &[rest_marker])?;
    let missing_key_name = list(&mut ctx, &runtime, &[key_marker, Word::NIL])?;
    for pattern in [missing_rest, missing_key_name] {
        let form = list(&mut ctx, &runtime, &[op, pattern, value])?;
        assert_eq!(
            call_macro(&runtime, &mut ctx, "DESTRUCTURING-BIND", form),
            Err(ObjectError::TypeError)
        );
    }
    Ok(())
}

#[test]
fn quasiquote_handles_nested_markers_vectors_and_bare_splice_error() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let op = symbol(&mut ctx, &runtime, "QUASIQUOTE")?;
    let unquote = symbol(&mut ctx, &runtime, "UNQUOTE")?;
    let splice = symbol(&mut ctx, &runtime, "UNQUOTE-SPLICING")?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;
    let a = symbol(&mut ctx, &runtime, "A")?;
    let unquoted = list(&mut ctx, &runtime, &[unquote, value])?;
    let datum = list(&mut ctx, &runtime, &[a, unquoted])?;
    let form = list(&mut ctx, &runtime, &[op, datum])?;
    let expanded = call_macro(&runtime, &mut ctx, "QUASIQUOTE", form)?;
    assert_eq!(
        elements(&mut ctx, expanded)?[0],
        symbol(&mut ctx, &runtime, "CONS")?
    );
    let spliced = list(&mut ctx, &runtime, &[splice, value])?;
    let bare_splice = list(&mut ctx, &runtime, &[op, spliced])?;
    assert_eq!(
        call_macro(&runtime, &mut ctx, "QUASIQUOTE", bare_splice),
        Err(ObjectError::TypeError)
    );
    let vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &[value])?;
    let vector_form = list(&mut ctx, &runtime, &[op, vector])?;
    let vector_expansion = call_macro(&runtime, &mut ctx, "QUASIQUOTE", vector_form)?;
    assert_eq!(
        elements(&mut ctx, vector_expansion)?[0],
        symbol(&mut ctx, &runtime, "APPLY")?
    );
    Ok(())
}

#[test]
fn multiple_value_helpers_capture_and_flatten_list_arguments() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let values = ncl_object::FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "VALUES")
            .ok_or(ObjectError::TypeError)?,
    )
    .map_err(|_| ObjectError::TypeError)?;
    let capture = ncl_object::FunctionObject::try_from(
        runtime
            .function(&mut ctx, "NCL-EXT", "CAPTURE-MULTIPLE-VALUES")
            .ok_or(ObjectError::TypeError)?,
    )
    .map_err(|_| ObjectError::TypeError)?;
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            values,
            &[Word::fixnum(1), Word::fixnum(2), Word::fixnum(3)],
        ),
        Ok(Word::fixnum(1))
    );
    let captured = runtime.call_builtin(&mut ctx, capture, &[])?;
    assert_eq!(
        elements(&mut ctx, captured)?,
        vec![Word::fixnum(1), Word::fixnum(2), Word::fixnum(3)]
    );

    let call_list = ncl_object::FunctionObject::try_from(
        runtime
            .function(&mut ctx, "NCL-EXT", "MULTIPLE-VALUE-CALL-LIST")
            .ok_or(ObjectError::TypeError)?,
    )
    .map_err(|_| ObjectError::TypeError)?;
    let first = list(&mut ctx, &runtime, &[Word::fixnum(4), Word::fixnum(5)])?;
    let second = list(&mut ctx, &runtime, &[Word::fixnum(6)])?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, call_list, &[values.as_word(), first, second]),
        Ok(Word::fixnum(4))
    );
    assert_eq!(
        ctx.values(),
        &[Word::fixnum(4), Word::fixnum(5), Word::fixnum(6)]
    );
    let dotted = ncl_object::make_cons(&mut ctx, &runtime, Word::fixnum(7), Word::fixnum(8))?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, call_list, &[values.as_word(), dotted]),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn quasiquote_expansion_handles_direct_splicing_nested_markers_and_bad_arity()
-> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let op = symbol(&mut ctx, &runtime, "QUASIQUOTE")?;
    let unquote = symbol(&mut ctx, &runtime, "UNQUOTE")?;
    let splice = symbol(&mut ctx, &runtime, "UNQUOTE-SPLICING")?;
    let nested = symbol(&mut ctx, &runtime, "QUASIQUOTE")?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;

    let direct_datum = list(&mut ctx, &runtime, &[unquote, value])?;
    let direct = list(&mut ctx, &runtime, &[op, direct_datum])?;
    assert_eq!(call_macro(&runtime, &mut ctx, "QUASIQUOTE", direct)?, value);

    let spliced = list(&mut ctx, &runtime, &[splice, value])?;
    let datum = list(&mut ctx, &runtime, &[spliced, value])?;
    let splicing_form = list(&mut ctx, &runtime, &[op, datum])?;
    let expansion = call_macro(&runtime, &mut ctx, "QUASIQUOTE", splicing_form)?;
    assert_eq!(
        elements(&mut ctx, expansion)?[0],
        symbol(&mut ctx, &runtime, "APPEND")?
    );

    let nested_unquote = list(&mut ctx, &runtime, &[unquote, value])?;
    let nested_datum = list(&mut ctx, &runtime, &[nested, nested_unquote])?;
    let nested_form = list(&mut ctx, &runtime, &[op, nested_datum])?;
    let nested_expansion = call_macro(&runtime, &mut ctx, "QUASIQUOTE", nested_form)?;
    assert_eq!(
        elements(&mut ctx, nested_expansion)?[0],
        symbol(&mut ctx, &runtime, "LIST")?
    );

    let bad_arity_short = list(&mut ctx, &runtime, &[op])?;
    assert_eq!(
        call_macro(&runtime, &mut ctx, "QUASIQUOTE", bad_arity_short),
        Err(ObjectError::TypeError)
    );
    let bad_arity_long = list(&mut ctx, &runtime, &[op, value, value])?;
    assert_eq!(
        call_macro(&runtime, &mut ctx, "QUASIQUOTE", bad_arity_long,),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn loop_parser_covers_directions_accumulators_conditionals_and_errors() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let x = symbol(&mut ctx, &runtime, "X")?;
    let for_word = symbol(&mut ctx, &runtime, "FOR")?;
    for direction in ["UPFROM", "DOWNFROM", "UPTO", "BELOW", "DOWNTO", "ABOVE"] {
        let direction_symbol = symbol(&mut ctx, &runtime, direction)?;
        let input = [for_word, x, direction_symbol, Word::fixnum(1)];
        let ast = super::super::r#loop::parse_loop(&mut ctx, &input)?;
        assert_eq!(ast.clauses.len(), 1);
    }
    let collect = symbol(&mut ctx, &runtime, "COLLECT")?;
    let into = symbol(&mut ctx, &runtime, "INTO")?;
    let ast = super::super::r#loop::parse_loop(&mut ctx, &[collect, x, into, x])?;
    assert!(matches!(
        ast.clauses[0],
        super::super::r#loop::LoopClause::Accumulate { .. }
    ));
    let then = symbol(&mut ctx, &runtime, "THEN")?;
    let bad_input = [for_word, x, then];
    assert_eq!(
        super::super::r#loop::parse_loop(&mut ctx, &bad_input),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

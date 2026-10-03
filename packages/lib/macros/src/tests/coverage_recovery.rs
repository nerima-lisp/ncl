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

fn assert_same_form(
    ctx: &mut ThreadContext,
    actual: Word,
    expected: Word,
) -> Result<(), ObjectError> {
    let actual_parts = elements(ctx, actual)?;
    let expected_parts = elements(ctx, expected)?;
    assert_eq!(actual_parts.len(), expected_parts.len());
    for (actual_part, expected_part) in actual_parts.into_iter().zip(expected_parts) {
        if actual_part.is_cons() || expected_part.is_cons() {
            assert_same_form(ctx, actual_part, expected_part)?;
        } else {
            assert_eq!(actual_part, expected_part);
        }
    }
    Ok(())
}

#[test]
fn control_macro_expanders_cover_handler_restart_and_multiple_value_forms(
) -> Result<(), ObjectError> {
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
fn restart_expanders_preserve_all_option_aliases_and_reject_incomplete_clauses(
) -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let restart = named(&mut ctx, &runtime, "RESTART")?;
    let function = named(&mut ctx, &runtime, "FUNCTION")?;
    let report = named(&mut ctx, &runtime, "REPORT")?;
    let interactive = named(&mut ctx, &runtime, "INTERACTIVE")?;
    let test = named(&mut ctx, &runtime, "TEST")?;
    let report_value = named(&mut ctx, &runtime, "REPORT-VALUE")?;
    let interactive_value = named(&mut ctx, &runtime, "INTERACTIVE-VALUE")?;
    let test_value = named(&mut ctx, &runtime, "TEST-VALUE")?;
    let body = named(&mut ctx, &runtime, "BODY")?;
    let clause = list_of(
        &mut ctx,
        &runtime,
        &[
            restart,
            function,
            report,
            report_value,
            interactive,
            interactive_value,
            test,
            test_value,
            body,
        ],
    )?;
    let clauses = list_of(&mut ctx, &runtime, &[clause])?;
    let restart_bind_name = named(&mut ctx, &runtime, "RESTART-BIND")?;
    let bind = list_of(&mut ctx, &runtime, &[restart_bind_name, clauses, body])?;
    let expanded = call_macro(&runtime, &mut ctx, "RESTART-BIND", bind)?;
    let outer = elements(&mut ctx, expanded)?;
    assert_eq!(outer[0], named(&mut ctx, &runtime, "LET")?);
    let binding_list = elements(&mut ctx, outer[1])?;
    let binding = elements(&mut ctx, binding_list[0])?;
    let push = elements(&mut ctx, binding[1])?;
    assert_eq!(push[0], named(&mut ctx, &runtime, "NCL-EXT::PUSH-RESTART")?);
    assert_eq!(push[3], report_value);
    assert_eq!(push[4], interactive_value);
    assert_eq!(push[5], test_value);

    let missing_clause = list_of(&mut ctx, &runtime, &[restart_bind_name])?;
    let incomplete_restart = list_of(&mut ctx, &runtime, &[restart])?;
    let incomplete_clauses = list_of(&mut ctx, &runtime, &[incomplete_restart])?;
    let incomplete_bind = list_of(&mut ctx, &runtime, &[restart_bind_name, incomplete_clauses])?;
    for malformed in [missing_clause, incomplete_bind] {
        assert_eq!(
            call_macro(&runtime, &mut ctx, "RESTART-BIND", malformed),
            Err(ObjectError::TypeError)
        );
    }

    let restart_case_name = named(&mut ctx, &runtime, "RESTART-CASE")?;
    let empty_case = list_of(&mut ctx, &runtime, &[restart_case_name, body])?;
    let empty_expansion = call_macro(&runtime, &mut ctx, "RESTART-CASE", empty_case)?;
    assert_eq!(
        elements(&mut ctx, empty_expansion)?[0],
        named(&mut ctx, &runtime, "BLOCK")?
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
fn control_boolean_and_cond_expanders_cover_empty_and_short_forms() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let and_name = named(&mut ctx, &runtime, "AND")?;
    let or_name = named(&mut ctx, &runtime, "OR")?;
    let cond_name = named(&mut ctx, &runtime, "COND")?;
    let value = named(&mut ctx, &runtime, "VALUE")?;
    let empty_and = list_of(&mut ctx, &runtime, &[and_name])?;
    let true_word = call_macro(&runtime, &mut ctx, "AND", empty_and)?;
    assert_eq!(true_word, Word::TRUE);
    let empty_or = list_of(&mut ctx, &runtime, &[or_name])?;
    let false_word = call_macro(&runtime, &mut ctx, "OR", empty_or)?;
    assert_eq!(false_word, Word::NIL);
    let single_and_form = list_of(&mut ctx, &runtime, &[and_name, value])?;
    let single_and = call_macro(&runtime, &mut ctx, "AND", single_and_form)?;
    assert_eq!(single_and, value);
    let single_or_form = list_of(&mut ctx, &runtime, &[or_name, value])?;
    let single_or = call_macro(&runtime, &mut ctx, "OR", single_or_form)?;
    assert_eq!(single_or, value);
    let empty_cond_form = list_of(&mut ctx, &runtime, &[cond_name])?;
    let empty_cond = call_macro(&runtime, &mut ctx, "COND", empty_cond_form)?;
    assert_eq!(empty_cond, Word::NIL);
    let clause = list_of(&mut ctx, &runtime, &[value])?;
    let one_cond_form = list_of(&mut ctx, &runtime, &[cond_name, clause])?;
    let one_cond = call_macro(&runtime, &mut ctx, "COND", one_cond_form)?;
    let parts = elements(&mut ctx, one_cond)?;
    assert_eq!(parts[0], named(&mut ctx, &runtime, "IF")?);
    assert_eq!(parts[1], value);
    assert_eq!(parts[2], value);
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
    assert!(parts[2..]
        .iter()
        .map(|part| elements(&mut ctx, *part))
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .any(|part| part.first() == Some(&defun)));
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
fn destructuring_key_patterns_expand_defaults_supplied_and_explicit_keywords(
) -> Result<(), ObjectError> {
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
fn destructuring_optional_rest_whole_and_environment_expand_to_checked_bindings(
) -> Result<(), ObjectError> {
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
fn destructuring_optional_rest_whole_and_environment_bindings_expand_distinctly(
) -> Result<(), ObjectError> {
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
fn destructuring_key_supplied_p_and_custom_keyword_expand_to_lookup_guards(
) -> Result<(), ObjectError> {
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
fn quasiquote_expansion_handles_direct_splicing_nested_markers_and_bad_arity(
) -> Result<(), ObjectError> {
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

#[test]
fn control_short_expansions_match_the_complete_constructor_shape() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let test = named(&mut ctx, &runtime, "TEST")?;
    let body = named(&mut ctx, &runtime, "BODY")?;
    let progn_name = named(&mut ctx, &runtime, "PROGN")?;
    let progn = list_of(&mut ctx, &runtime, &[progn_name, body])?;
    let cases = [
        ("WHEN", vec!["IF"], vec![test, progn, Word::NIL]),
        ("UNLESS", vec!["IF"], vec![test, Word::NIL, progn]),
        ("AND", vec!["IF"], vec![test, body, Word::NIL]),
        (
            "RETURN-FROM",
            vec!["RETURN-FROM"],
            vec![Word::NIL, Word::NIL],
        ),
    ];
    for (name, expected_head, expected_tail) in cases {
        let operator = named(&mut ctx, &runtime, name)?;
        let form = match name {
            "WHEN" | "UNLESS" | "AND" => list_of(&mut ctx, &runtime, &[operator, test, body])?,
            "COND" => {
                let clause = list_of(&mut ctx, &runtime, &[test])?;
                list_of(&mut ctx, &runtime, &[operator, clause])?
            }
            "RETURN-FROM" => list_of(&mut ctx, &runtime, &[operator])?,
            _ => return Err(ObjectError::TypeError),
        };
        let expanded = if name == "RETURN-FROM" {
            call_macro(&runtime, &mut ctx, "RETURN", form)?
        } else {
            call_macro(&runtime, &mut ctx, name, form)?
        };
        let mut expected = vec![named(&mut ctx, &runtime, expected_head[0])?];
        expected.extend(expected_tail);
        let expected = list_of(&mut ctx, &runtime, &expected)?;
        assert_same_form(&mut ctx, expanded, expected)?;
    }
    Ok(())
}

#[test]
fn quasiquote_constructor_shapes_are_exact_for_atoms_lists_and_vectors() -> Result<(), ObjectError>
{
    let (runtime, mut ctx) = fixture()?;
    let op = named(&mut ctx, &runtime, "QUASIQUOTE")?;
    let value = named(&mut ctx, &runtime, "VALUE")?;
    let a = named(&mut ctx, &runtime, "A")?;
    let b = named(&mut ctx, &runtime, "B")?;
    let quote = named(&mut ctx, &runtime, "QUOTE")?;
    let cons = named(&mut ctx, &runtime, "CONS")?;
    let nil_quote = list_of(&mut ctx, &runtime, &[quote, Word::NIL])?;
    let quote_a = list_of(&mut ctx, &runtime, &[quote, a])?;
    let quote_b = list_of(&mut ctx, &runtime, &[quote, b])?;
    let cons_b = list_of(&mut ctx, &runtime, &[cons, quote_b, nil_quote])?;
    let expected_list = list_of(&mut ctx, &runtime, &[cons, quote_a, cons_b])?;
    let list_datum = list_of(&mut ctx, &runtime, &[a, b])?;
    let quote_value = list_of(&mut ctx, &runtime, &[quote, value])?;
    let cases = [(value, quote_value), (list_datum, expected_list)];
    for (datum, expected) in cases {
        let form = list_of(&mut ctx, &runtime, &[op, datum])?;
        let expanded = call_macro(&runtime, &mut ctx, "QUASIQUOTE", form)?;
        assert_same_form(&mut ctx, expanded, expected)?;
    }

    let unquote = named(&mut ctx, &runtime, "UNQUOTE")?;
    let splice = named(&mut ctx, &runtime, "UNQUOTE-SPLICING")?;
    let append = named(&mut ctx, &runtime, "APPEND")?;
    let spliced = list_of(&mut ctx, &runtime, &[splice, value])?;
    let datum = list_of(&mut ctx, &runtime, &[spliced, a])?;
    let expected_tail = list_of(&mut ctx, &runtime, &[cons, quote_a, nil_quote])?;
    let expected = list_of(&mut ctx, &runtime, &[append, value, expected_tail])?;
    let form = list_of(&mut ctx, &runtime, &[op, datum])?;
    let expanded = call_macro(&runtime, &mut ctx, "QUASIQUOTE", form)?;
    assert_same_form(&mut ctx, expanded, expected)?;

    let vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &[value, a])?;
    let vector_form = list_of(&mut ctx, &runtime, &[op, vector])?;
    let vector_code_tail = list_of(&mut ctx, &runtime, &[cons, quote_a, nil_quote])?;
    let vector_code = list_of(&mut ctx, &runtime, &[cons, quote_value, vector_code_tail])?;
    let function = named(&mut ctx, &runtime, "FUNCTION")?;
    let vector_name = named(&mut ctx, &runtime, "VECTOR")?;
    let apply = named(&mut ctx, &runtime, "APPLY")?;
    let vector_function = list_of(&mut ctx, &runtime, &[function, vector_name])?;
    let expected = list_of(&mut ctx, &runtime, &[apply, vector_function, vector_code])?;
    let expanded = call_macro(&runtime, &mut ctx, "QUASIQUOTE", vector_form)?;
    assert_same_form(&mut ctx, expanded, expected)?;
    let unquoted = list_of(&mut ctx, &runtime, &[unquote, value])?;
    let direct = list_of(&mut ctx, &runtime, &[op, unquoted])?;
    assert_eq!(call_macro(&runtime, &mut ctx, "QUASIQUOTE", direct)?, value);
    Ok(())
}

#[test]
fn control_expanders_cover_remaining_structural_forms_table_driven() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let test = named(&mut ctx, &runtime, "TEST")?;
    let body = named(&mut ctx, &runtime, "BODY")?;
    let x = named(&mut ctx, &runtime, "X")?;
    let init = named(&mut ctx, &runtime, "INIT")?;
    let step = named(&mut ctx, &runtime, "STEP")?;
    let tag = named(&mut ctx, &runtime, "TAG")?;

    let when = named(&mut ctx, &runtime, "WHEN")?;
    let unless = named(&mut ctx, &runtime, "UNLESS")?;
    let when_form = list_of(&mut ctx, &runtime, &[when, test, body, init])?;
    let unless_form = list_of(&mut ctx, &runtime, &[unless, test, body])?;
    let prog_variable = list_of(&mut ctx, &runtime, &[x, init])?;
    let prog_variables = list_of(&mut ctx, &runtime, &[prog_variable])?;
    let prog = named(&mut ctx, &runtime, "PROG")?;
    let prog_form = list_of(&mut ctx, &runtime, &[prog, prog_variables, tag])?;
    let do_variable = list_of(&mut ctx, &runtime, &[x, init, step])?;
    let do_variables = list_of(&mut ctx, &runtime, &[do_variable])?;
    let end_clause = list_of(&mut ctx, &runtime, &[test, body])?;
    let nth_value = named(&mut ctx, &runtime, "NTH-VALUE")?;
    let nth_value_form = list_of(&mut ctx, &runtime, &[nth_value, Word::fixnum(1), body])?;

    let cases = [
        ("WHEN", when_form, "IF"),
        ("UNLESS", unless_form, "IF"),
        ("PROG", prog_form, "BLOCK"),
        ("NTH-VALUE", nth_value_form, "MULTIPLE-VALUE-CALL"),
    ];
    for (name, form, expected_head) in cases {
        let expanded = call_macro(&runtime, &mut ctx, name, form)?;
        assert_eq!(
            elements(&mut ctx, expanded)?.first().copied(),
            Some(named(&mut ctx, &runtime, expected_head)?),
            "{name} expansion head"
        );
    }

    let do_star = named(&mut ctx, &runtime, "DO*")?;
    let let_star = named(&mut ctx, &runtime, "LET*")?;
    let do_star_form = list_of(
        &mut ctx,
        &runtime,
        &[do_star, do_variables, end_clause, tag],
    )?;
    let do_star_expansion = call_macro(&runtime, &mut ctx, "DO*", do_star_form)?;
    let do_star = elements(&mut ctx, do_star_expansion)?;
    let do_star_body = elements(&mut ctx, do_star[2])?;
    assert_eq!(do_star_body[0], let_star);
    Ok(())
}

#[test]
fn quasiquote_vectors_and_nested_splicing_preserve_constructor_shapes() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let op = named(&mut ctx, &runtime, "QUASIQUOTE")?;
    let splice = named(&mut ctx, &runtime, "UNQUOTE-SPLICING")?;
    let nested = named(&mut ctx, &runtime, "QUASIQUOTE")?;
    let value = named(&mut ctx, &runtime, "VALUE")?;
    let spliced = list_of(&mut ctx, &runtime, &[splice, value])?;
    let vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &[spliced, value])?;
    let vector_form = list_of(&mut ctx, &runtime, &[op, vector])?;
    let vector_expansion = call_macro(&runtime, &mut ctx, "QUASIQUOTE", vector_form)?;
    let vector_parts = elements(&mut ctx, vector_expansion)?;
    assert_eq!(vector_parts[0], named(&mut ctx, &runtime, "APPLY")?);
    let function_form = elements(&mut ctx, vector_parts[1])?;
    assert_eq!(
        function_form,
        vec![
            named(&mut ctx, &runtime, "FUNCTION")?,
            named(&mut ctx, &runtime, "VECTOR")?
        ]
    );

    let nested_datum = list_of(&mut ctx, &runtime, &[splice, value])?;
    let nested_datum_form = list_of(&mut ctx, &runtime, &[nested, nested_datum])?;
    let nested_form = list_of(&mut ctx, &runtime, &[op, nested_datum_form])?;
    let list = named(&mut ctx, &runtime, "LIST")?;
    let quote = named(&mut ctx, &runtime, "QUOTE")?;
    let nested_expansion = call_macro(&runtime, &mut ctx, "QUASIQUOTE", nested_form)?;
    let nested_parts = elements(&mut ctx, nested_expansion)?;
    assert_eq!(nested_parts[0], list);
    assert_eq!(elements(&mut ctx, nested_parts[1])?, vec![quote, nested]);
    assert_eq!(nested_parts.len(), 3);
    Ok(())
}

#[test]
fn destructuring_body_marker_binds_the_remaining_arguments() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let op = symbol(&mut ctx, &runtime, "DESTRUCTURING-BIND")?;
    let body_marker = symbol(&mut ctx, &runtime, "&BODY")?;
    let body = symbol(&mut ctx, &runtime, "BODY")?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;
    let pattern = list(&mut ctx, &runtime, &[body_marker, body])?;
    let form = list(&mut ctx, &runtime, &[op, pattern, value, body])?;

    let expanded = call_macro(&runtime, &mut ctx, "DESTRUCTURING-BIND", form)?;
    let parts = elements(&mut ctx, expanded)?;
    let bindings = elements(&mut ctx, parts[1])?;
    let source_binding = elements(&mut ctx, bindings[0])?;
    assert_eq!(parts[0], symbol(&mut ctx, &runtime, "LET*")?);
    assert_eq!(source_binding[1], value);
    assert_eq!(
        elements(&mut ctx, bindings[1])?,
        vec![body, source_binding[0]]
    );
    assert_eq!(parts[2], body);
    Ok(())
}

#[test]
fn quasiquote_treats_noncanonical_marker_lists_as_data() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let op = symbol(&mut ctx, &runtime, "QUASIQUOTE")?;
    let unquote = symbol(&mut ctx, &runtime, "UNQUOTE")?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;
    let extra = symbol(&mut ctx, &runtime, "EXTRA")?;
    let malformed = list(&mut ctx, &runtime, &[unquote, value, extra])?;
    let form = list(&mut ctx, &runtime, &[op, malformed])?;

    let expanded = call_macro(&runtime, &mut ctx, "QUASIQUOTE", form)?;
    let parts = elements(&mut ctx, expanded)?;
    assert_eq!(parts[0], symbol(&mut ctx, &runtime, "CONS")?);
    let quoted_marker = elements(&mut ctx, parts[1])?;
    assert_eq!(quoted_marker[0], symbol(&mut ctx, &runtime, "QUOTE")?);
    assert_eq!(quoted_marker[1], unquote);
    let tail = elements(&mut ctx, parts[2])?;
    assert_eq!(tail[0], symbol(&mut ctx, &runtime, "CONS")?);
    assert_eq!(elements(&mut ctx, tail[1])?[1], value);
    let final_tail = elements(&mut ctx, tail[2])?;
    assert_eq!(final_tail[0], symbol(&mut ctx, &runtime, "CONS")?);
    assert_eq!(elements(&mut ctx, final_tail[1])?[1], extra);
    Ok(())
}

#[test]
fn do_and_prog_star_expansions_use_their_sequential_forms() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let x = named(&mut ctx, &runtime, "X")?;
    let init = named(&mut ctx, &runtime, "INIT")?;
    let step = named(&mut ctx, &runtime, "STEP")?;
    let test = named(&mut ctx, &runtime, "TEST")?;
    let result = named(&mut ctx, &runtime, "RESULT")?;
    let body = named(&mut ctx, &runtime, "BODY")?;
    let variable = list_of(&mut ctx, &runtime, &[x, init, step])?;
    let variables = list_of(&mut ctx, &runtime, &[variable])?;
    let end = list_of(&mut ctx, &runtime, &[test, result])?;

    let do_star = named(&mut ctx, &runtime, "DO*")?;
    let do_star_form = list_of(&mut ctx, &runtime, &[do_star, variables, end, body])?;
    let do_star_expansion = call_macro(&runtime, &mut ctx, "DO*", do_star_form)?;
    let do_star_parts = elements(&mut ctx, do_star_expansion)?;
    let do_star_let = elements(&mut ctx, do_star_parts[2])?;
    let do_star_tagbody = elements(&mut ctx, do_star_let[2])?;
    assert_eq!(do_star_let[0], named(&mut ctx, &runtime, "LET*")?);
    assert_eq!(
        elements(&mut ctx, do_star_tagbody[4])?[0],
        named(&mut ctx, &runtime, "SETQ")?
    );

    let prog_star = named(&mut ctx, &runtime, "PROG*")?;
    let prog_variable = list_of(&mut ctx, &runtime, &[x, init])?;
    let prog_variables = list_of(&mut ctx, &runtime, &[prog_variable])?;
    let prog_star_form = list_of(&mut ctx, &runtime, &[prog_star, prog_variables, body])?;
    let prog_star_expansion = call_macro(&runtime, &mut ctx, "PROG*", prog_star_form)?;
    let prog_star_parts = elements(&mut ctx, prog_star_expansion)?;
    let prog_star_body = elements(&mut ctx, prog_star_parts[2])?;
    assert_eq!(prog_star_body[0], named(&mut ctx, &runtime, "LET*")?);
    Ok(())
}

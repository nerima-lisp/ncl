use super::*;

fn fixture() -> Result<(Runtime, ThreadContext), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    Ok((runtime, ctx))
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
    assert!(elements(&mut ctx, parts[1])?.len() >= 6);
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

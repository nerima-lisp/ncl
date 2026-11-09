use ncl_hir::{LocalFunctionKind, NodeKind};
use ncl_ir::{Constant, OpKind, Terminator as MirTerminator};
use ncl_object::Word;

use super::common::{TestResult, context, expand_build, lower_both_and_verify};

#[test]
fn lambda_optional_keyword_rest_and_aux_modes_lower_to_distinct_argument_paths() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(121);
    let optional = fixture.cl("&OPTIONAL");
    let first = fixture.user("FIRST");
    let first_supplied = fixture.user("FIRST-P");
    let first_spec = fixture.list(&[first, Word::fixnum(5), first_supplied]);
    let second = fixture.user("SECOND");
    let second_supplied = fixture.user("SECOND-P");
    let second_spec = fixture.list(&[second, first, second_supplied]);
    let rest_marker = fixture.cl("&REST");
    let rest = fixture.user("REST");
    let key_marker = fixture.cl("&KEY");
    let alias = fixture.keyword("ALIAS");
    let key_value = fixture.user("KEY-VALUE");
    let key_parameter = fixture.list(&[alias, key_value]);
    let key_supplied = fixture.user("KEY-P");
    let key_spec = fixture.list(&[key_parameter, second, key_supplied]);
    let allow_other_keys = fixture.cl("&ALLOW-OTHER-KEYS");
    let aux_marker = fixture.cl("&AUX");
    let copy = fixture.user("COPY");
    let copy_spec = fixture.list(&[copy, key_value]);
    let lambda_list = fixture.list(&[
        optional,
        first_spec,
        second_spec,
        rest_marker,
        rest,
        key_marker,
        key_spec,
        allow_other_keys,
        aux_marker,
        copy_spec,
    ]);
    let list_name = fixture.cl("LIST");
    let list_function = fixture.form("FUNCTION", &[list_name]);
    let body = fixture.form(
        "FUNCALL",
        &[
            list_function,
            first,
            first_supplied,
            second,
            second_supplied,
            rest,
            key_value,
            key_supplied,
            copy,
        ],
    );
    let input = fixture.form("LAMBDA", &[lambda_list, body]);
    let source = "(lambda (&optional (first 5 first-p) (second first second-p) &rest rest &key ((:alias key-value) second key-p) &allow-other-keys &aux (copy key-value)) (funcall #'list first first-p second second-p rest key-value key-p copy))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let signature = form
        .lambda_signatures()
        .values()
        .next()
        .ok_or_else(|| std::io::Error::other("lambda signature is missing"))?;
    assert_eq!(signature.optional.len(), 2);
    assert!(signature.rest.is_some());
    assert_eq!(signature.keys.len(), 1);
    assert!(signature.allow_other_keys);
    assert_eq!(signature.aux.len(), 1);

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let function = lowered
        .nested
        .first()
        .ok_or_else(|| std::io::Error::other("lambda MIR is missing"))?;
    let operations = function.blocks.iter().flat_map(|block| &block.ops);
    assert_eq!(
        function
            .blocks
            .iter()
            .filter(|block| matches!(&block.terminator, MirTerminator::Branch { .. }))
            .count(),
        3
    );
    for name in [
        "make-rest-list",
        "check-keywords",
        "keyword-value",
        "keyword-supplied-p",
    ] {
        assert!(operations.clone().any(|operation| matches!(
            &operation.kind,
            OpKind::Builtin { name: actual, .. } if actual == name
        )));
    }
    assert!(function.constants.contains(&Constant::Fixnum(5)));
    assert!(function.constants.contains(&Constant::Fixnum(2)));
    assert!(function.constants.contains(&Constant::T));
    assert!(function.constants.contains(&Constant::Nil));

    // CLHS lambda lists bind supplied-p variables and evaluate optional, keyword, and auxiliary defaults.
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_da.htm
    Ok(())
}

#[test]
fn nested_lambda_captures_only_used_variables_and_a_local_function() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(123);
    let first = fixture.user("FIRST");
    let second = fixture.user("SECOND");
    let unused = fixture.user("UNUSED");
    let first_binding = fixture.list(&[first, Word::fixnum(3)]);
    let second_binding = fixture.list(&[second, Word::fixnum(5)]);
    let unused_binding = fixture.list(&[unused, Word::fixnum(7)]);
    let bindings = fixture.list(&[first_binding, second_binding, unused_binding]);

    let helper_name = fixture.user("HELPER");
    let argument = fixture.user("ARGUMENT");
    let helper_lambda_list = fixture.list(&[argument]);
    let addition = fixture.form("+", &[argument, first]);
    let helper_definition = fixture.list(&[helper_name, helper_lambda_list, addition]);
    let definitions = fixture.list(&[helper_definition]);

    let empty_lambda_list = fixture.list(&[]);
    let list_name = fixture.cl("LIST");
    let list_function = fixture.form("FUNCTION", &[list_name]);
    let helper_reference = fixture.form("FUNCTION", &[helper_name]);
    let body = fixture.form("FUNCALL", &[list_function, first, second, helper_reference]);
    let lambda = fixture.form("LAMBDA", &[empty_lambda_list, body]);
    let lambda_call = fixture.form("FUNCALL", &[lambda]);
    let flet = fixture.form("FLET", &[definitions, lambda_call]);
    let input = fixture.form("LET", &[bindings, flet]);
    let source = "(let ((first 3) (second 5) (unused 7)) (flet ((helper (argument) (+ argument first))) (funcall (lambda () (funcall #'list first second #'helper)))))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let captured_counts = lowered
        .entry
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter_map(|operation| match &operation.kind {
            OpKind::MakeClosure { captures, .. } => Some(captures.len()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(captured_counts.contains(&1));
    assert!(captured_counts.contains(&3));
    assert!(!captured_counts.contains(&4));
    assert!(lowered.nested.iter().any(|function| {
        let captures = function
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .filter_map(|operation| match &operation.kind {
                OpKind::LoadCapture { index } => Some(*index),
                _ => None,
            })
            .collect::<Vec<_>>();
        [0, 1, 2].iter().all(|index| captures.contains(index))
    }));
    assert!(!lowered.entry.blocks.iter().any(|block| {
        block
            .ops
            .iter()
            .any(|operation| matches!(&operation.kind, OpKind::MakeValueCell { .. }))
    }));

    // CLHS FLET has lexical function scope, and closures retain referenced lexical bindings.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_flet_.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_ad.htm
    Ok(())
}

#[test]
fn sibling_lambdas_share_a_mutated_outer_binding_cell() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(133);
    let value = fixture.user("VALUE");
    let value_binding = fixture.list(&[value, Word::fixnum(0)]);
    let value_bindings = fixture.list(&[value_binding]);

    let writer = fixture.user("WRITER");
    let reader = fixture.user("READER");
    let lambda_list = fixture.list(&[]);
    let assignment = fixture.form("SETQ", &[value, Word::fixnum(1)]);
    let writer_lambda = fixture.form("LAMBDA", &[lambda_list, assignment]);
    let reader_lambda = fixture.form("LAMBDA", &[lambda_list, value]);
    let writer_binding = fixture.list(&[writer, writer_lambda]);
    let reader_binding = fixture.list(&[reader, reader_lambda]);
    let closures = fixture.list(&[writer_binding, reader_binding]);
    let call_writer = fixture.form("FUNCALL", &[writer]);
    let call_reader = fixture.form("FUNCALL", &[reader]);
    let body = fixture.form("PROGN", &[call_writer, call_reader]);
    let inner_let = fixture.form("LET", &[closures, body]);
    let input = fixture.form("LET", &[value_bindings, inner_let]);
    let source = "(let ((value 0))\n  (let ((writer (lambda () (setq value 1)))\n        (reader (lambda () value)))\n    (funcall writer)\n    (funcall reader)))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let captures = lowered
        .entry
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter_map(|operation| match &operation.kind {
            OpKind::MakeClosure { captures, .. } => Some(captures),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(captures.len(), 2);
    assert!(captures.iter().all(|capture| capture.len() == 1));
    assert_eq!(captures[0][0], captures[1][0]);

    let cell = lowered
        .entry
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|operation| match &operation.kind {
            OpKind::MakeValueCell { .. } => operation.results.first().map(|(value, _)| *value),
            _ => None,
        })
        .ok_or_else(|| std::io::Error::other("outer lexical cell is missing"))?;
    assert_eq!(captures[0][0], cell);
    let mut writes_capture = false;
    let mut reads_capture = false;
    for function in &lowered.nested {
        let captured_cell = function
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .find_map(|operation| match &operation.kind {
                OpKind::LoadCapture { index: 0 } => {
                    operation.results.first().map(|(value, _)| *value)
                }
                _ => None,
            });
        let Some(captured_cell) = captured_cell else {
            continue;
        };
        let mut operations = function.blocks.iter().flat_map(|block| &block.ops);
        writes_capture |= operations.clone().any(|operation| {
            matches!(
                &operation.kind,
                OpKind::StoreField { object, field: 0, value }
                    if *object == captured_cell
                        && super::common::constant_for_value(function, *value)
                            == Some(&Constant::Fixnum(1))
            )
        });
        reads_capture |= operations.any(|operation| {
            matches!(
                &operation.kind,
                OpKind::LoadField { object, field: 0 } if *object == captured_cell
            )
        });
    }
    assert!(writes_capture);
    assert!(reads_capture);

    // CLHS lexical closures share the captured binding, so mutation is visible to sibling closures.
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_ad.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_setq.htm
    Ok(())
}

#[test]
fn absent_optional_keyword_and_aux_defaults_lower_to_nil_paths() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(126);
    let optional_marker = fixture.cl("&OPTIONAL");
    let optional = fixture.user("OPTIONAL-VALUE");
    let key_marker = fixture.cl("&KEY");
    let key = fixture.user("KEY-VALUE");
    let aux_marker = fixture.cl("&AUX");
    let aux = fixture.user("AUX-VALUE");
    let lambda_list = fixture.list(&[optional_marker, optional, key_marker, key, aux_marker, aux]);
    let body = fixture.form("LIST", &[optional, key, aux]);
    let input = fixture.form("LAMBDA", &[lambda_list, body]);
    let source = "(lambda (&optional optional-value &key key-value &aux aux-value) (list optional-value key-value aux-value))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let signature = form
        .lambda_signatures()
        .values()
        .next()
        .ok_or_else(|| std::io::Error::other("lambda signature is missing"))?;
    assert_eq!(signature.optional.len(), 1);
    assert_eq!(signature.keys.len(), 1);
    assert_eq!(signature.aux.len(), 1);
    assert!(!signature.allow_other_keys);

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let function = lowered
        .nested
        .first()
        .ok_or_else(|| std::io::Error::other("lambda MIR is missing"))?;
    let operations = function.blocks.iter().flat_map(|block| &block.ops);
    assert_eq!(
        function
            .blocks
            .iter()
            .filter(|block| matches!(&block.terminator, MirTerminator::Branch { .. }))
            .count(),
        2
    );
    for name in [
        "make-rest-list",
        "check-keywords",
        "keyword-value",
        "keyword-supplied-p",
    ] {
        assert!(operations.clone().any(|operation| matches!(
            &operation.kind,
            OpKind::Builtin { name: actual, .. } if actual == name
        )));
    }
    assert!(function.constants.contains(&Constant::Nil));

    // CLHS optional, keyword, and auxiliary parameters default to NIL when no initform is supplied.
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_da.htm
    Ok(())
}

#[test]
fn rest_only_lambda_builds_a_rest_list_without_keyword_validation() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(127);
    let required = fixture.user("REQUIRED");
    let rest_marker = fixture.cl("&REST");
    let rest = fixture.user("REST");
    let lambda_list = fixture.list(&[required, rest_marker, rest]);
    let body = fixture.form("LIST", &[required, rest]);
    let input = fixture.form("LAMBDA", &[lambda_list, body]);
    let source = "(lambda (required &rest rest) (list required rest))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let signature = form
        .lambda_signatures()
        .values()
        .next()
        .ok_or_else(|| std::io::Error::other("lambda signature is missing"))?;
    assert_eq!(signature.required.len(), 1);
    assert!(signature.rest.is_some());
    assert!(signature.keys.is_empty());

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let function = lowered
        .nested
        .first()
        .ok_or_else(|| std::io::Error::other("lambda MIR is missing"))?;
    let mut operations = function.blocks.iter().flat_map(|block| &block.ops);
    assert!(operations.clone().any(|operation| matches!(
        &operation.kind,
        OpKind::Builtin { name, .. } if name == "make-rest-list"
    )));
    assert!(!operations.any(|operation| matches!(
        &operation.kind,
        OpKind::Builtin { name, .. } if name == "check-keywords"
    )));
    assert!(
        function
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .any(|operation| { matches!(&operation.kind, OpKind::LoadArg { index: 1 }) })
    );

    // CLHS binds the rest parameter to a list of arguments after required parameters.
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_da.htm
    Ok(())
}

#[test]
fn nested_lambda_captures_a_recursive_labels_function_cell() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(128);
    let function_name = fixture.user("COUNT-DOWN");
    let parameter = fixture.user("N");
    let lambda_list = fixture.list(&[parameter]);
    let equal = fixture.form("=", &[parameter, Word::fixnum(0)]);
    let decrement = fixture.form("-", &[parameter, Word::fixnum(1)]);
    let recursive_call = fixture.form("COUNT-DOWN", &[decrement]);
    let body = fixture.form("IF", &[equal, Word::fixnum(0), recursive_call]);
    let definition = fixture.list(&[function_name, lambda_list, body]);
    let definitions = fixture.list(&[definition]);
    let empty_lambda_list = fixture.list(&[]);
    let function_reference = fixture.form("FUNCTION", &[function_name]);
    let lambda = fixture.form("LAMBDA", &[empty_lambda_list, function_reference]);
    let invoke = fixture.form("FUNCALL", &[lambda]);
    let input = fixture.form("LABELS", &[definitions, invoke]);
    let source = "(labels ((count-down (n) (if (= n 0) 0 (count-down (- n 1))))) (funcall (lambda () #'count-down)))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::LocalFunctions {
            kind: LocalFunctionKind::Labels,
            definitions,
            ..
        } if definitions.len() == 1
    )));

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    assert!(lowered.nested.iter().any(|function| {
        let mut operations = function.blocks.iter().flat_map(|block| &block.ops);
        let captures_function = operations
            .clone()
            .any(|operation| matches!(&operation.kind, OpKind::LoadCapture { .. }));
        let loads_function_cell =
            operations.any(|operation| matches!(&operation.kind, OpKind::LoadField { .. }));
        captures_function && loads_function_cell
    }));

    // CLHS LABELS gives its local functions recursive lexical scope that nested closures can retain.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_labels.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_ad.htm
    Ok(())
}

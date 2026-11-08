use ncl_hir::{CallTarget, MultipleValueOperation, NodeKind};
use ncl_ir::{Constant, OpKind};
use ncl_object::Word;

use super::common::{
    TestResult, constant_conditional_signature, context, expand_build, fixture_text, has_node,
    lower_both_and_verify,
};

#[test]
fn expanded_sequence_calls_and_conditional_match_baseline_outcomes() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(31);
    let add = fixture.form("+", &[Word::fixnum(1), Word::fixnum(2)]);
    let truth = fixture.cl("T");
    let conditional = fixture.form("IF", &[truth, Word::fixnum(3), Word::fixnum(4)]);
    let input = fixture.form("PROGN", &[add, conditional]);
    let source = fixture_text(
        include_str!("../fixtures/hir-bridge/core.lisp"),
        "(progn\n  (+ 1 2)\n  (if t 3 4))\n",
    );
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::Sequence { .. }
    )));
    assert!(has_node(&form, |node| matches!(node, NodeKind::If { .. })));
    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::Call {
            target: CallTarget::Global(_),
            ..
        }
    )));
    let (hir, baseline) = lower_both_and_verify(&builder, &form, &expr)?;
    let hir_outcome = constant_conditional_signature(&hir.entry);
    let baseline_outcome = constant_conditional_signature(&baseline.entry);
    assert_eq!(hir_outcome, baseline_outcome);
    assert_eq!(hir_outcome, Some((true, 3, 4)));
    Ok(())
}

#[test]
fn zero_argument_funcall_lowers_zero_argc_in_mir() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(30);
    let input = fixture.form("FUNCALL", &[]);
    let source = fixture_text(
        include_str!("../fixtures/hir-bridge/funcall-zero.lisp"),
        "(funcall)\n",
    );
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::Call {
            target: CallTarget::Global(_),
            arguments
        } if arguments.is_empty()
    )));
    let (hir, _) = lower_both_and_verify(&builder, &form, &expr)?;
    // CLHS FUNCALL syntax requires a function designator before its rest arguments.
    // Preserve argc=0 so the runtime sees the missing function designator.
    // https://www.lispworks.com/documentation/HyperSpec/Body/f_funcal.htm
    let argc = hir
        .entry
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|operation| match &operation.kind {
            OpKind::CallClosure {
                args,
                named_symbol: Some(_),
                ..
            } if args.len() == 1 => args.first().copied(),
            _ => None,
        })
        .ok_or_else(|| std::io::Error::other("zero-argument FUNCALL call is missing"))?;
    let argc_raw = hir
        .entry
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|operation| {
            if operation.results.iter().any(|(result, _)| *result == argc)
                && let OpKind::Convert {
                    op: ncl_ir::Convert::I64ToWord,
                    value,
                } = &operation.kind
            {
                Some(*value)
            } else {
                None
            }
        })
        .ok_or_else(|| std::io::Error::other("zero-argument FUNCALL argc is not converted"))?;
    let argc_constant = hir
        .entry
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|operation| {
            if operation
                .results
                .iter()
                .any(|(result, _)| *result == argc_raw)
                && let OpKind::Const { result } = &operation.kind
            {
                hir.entry.constants.get(usize::try_from(result.0).ok()?)
            } else {
                None
            }
        });
    assert_eq!(argc_constant, Some(&Constant::Fixnum(0)));
    Ok(())
}

#[test]
fn local_bindings_and_computed_calls_keep_function_identity() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(32);
    let x = fixture.user("X");
    let binding = fixture.list(&[x, Word::fixnum(4)]);
    let bindings = fixture.list(&[binding]);
    let function_name = fixture.user("INC");
    let argument = fixture.user("Y");
    let lambda_list = fixture.list(&[argument]);
    let increment = fixture.form("+", &[argument, Word::fixnum(1)]);
    let definition = fixture.list(&[function_name, lambda_list, increment]);
    let definitions = fixture.list(&[definition]);
    let function = fixture.form("FUNCTION", &[function_name]);
    let call = fixture.form("FUNCALL", &[function, x]);
    let flet = fixture.form("FLET", &[definitions, call]);
    let input = fixture.form("LET", &[bindings, flet]);
    let source = fixture_text(
        include_str!("../fixtures/hir-bridge/bindings.lisp"),
        "(let ((x 4))\n  (flet ((inc (y) (+ y 1)))\n    (funcall #'inc x)))\n",
    );
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(has_node(&form, |node| matches!(node, NodeKind::Let { .. })));
    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::LocalFunctions { .. }
    )));
    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::FunctionReference {
            target: CallTarget::Local(_)
        }
    )));
    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::Call {
            target: CallTarget::Value(_),
            ..
        }
    )));
    lower_both_and_verify(&builder, &form, &expr)?;
    Ok(())
}

#[test]
fn lambda_defaults_retain_ordered_parameter_scopes() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(33);
    let x = fixture.user("X");
    let optional = fixture.cl("&OPTIONAL");
    let y = fixture.user("Y");
    let y_supplied = fixture.user("Y-P");
    let optional_spec = fixture.list(&[y, x, y_supplied]);
    let rest = fixture.cl("&REST");
    let tail = fixture.user("TAIL");
    let key = fixture.cl("&KEY");
    let size = fixture.user("SIZE");
    let size_supplied = fixture.user("SIZE-P");
    let key_spec = fixture.list(&[size, Word::fixnum(8), size_supplied]);
    let allow = fixture.cl("&ALLOW-OTHER-KEYS");
    let aux = fixture.cl("&AUX");
    let copy = fixture.user("COPY");
    let aux_spec = fixture.list(&[copy, y]);
    let lambda_list = fixture.list(&[
        x,
        optional,
        optional_spec,
        rest,
        tail,
        key,
        key_spec,
        allow,
        aux,
        aux_spec,
    ]);
    let lambda = fixture.cl("LAMBDA");
    let input = fixture.list(&[lambda, lambda_list, copy]);
    let source = fixture_text(
        include_str!("../fixtures/hir-bridge/lambda-signature.lisp"),
        "(lambda (x &optional (y x y-p) &rest tail &key (size 8 size-p)\n         &allow-other-keys &aux (copy y))\n  copy)\n",
    );
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;
    let Some(signature) = form.lambda_signatures().values().next() else {
        return Err(std::io::Error::other("lambda signature is missing").into());
    };

    assert_eq!(signature.required.len(), 1);
    assert_eq!(signature.optional.len(), 1);
    assert!(signature.optional[0].default.is_some());
    assert!(signature.rest.is_some());
    assert_eq!(signature.keys.len(), 1);
    assert!(signature.allow_other_keys);
    assert_eq!(signature.aux.len(), 1);
    lower_both_and_verify(&builder, &form, &expr)?;
    Ok(())
}

#[test]
fn multiple_value_call_preserves_values_through_hir_lowering() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(43);
    let list_name = fixture.cl("LIST");
    let function = fixture.form("FUNCTION", &[list_name]);
    let values = fixture.form("VALUES", &[Word::fixnum(1), Word::fixnum(2)]);
    let input = fixture.form("MULTIPLE-VALUE-CALL", &[function, values]);
    let source = fixture_text(
        include_str!("../fixtures/hir-bridge/multiple-value-call.lisp"),
        "(multiple-value-call #'list\n  (values 1 2))\n",
    );
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::MultipleValues(MultipleValueOperation::Call { .. })
    )));
    assert!(has_node(
        &form,
        |node| matches!(node, NodeKind::MultipleValues(MultipleValueOperation::Produce { values }) if values.len() == 2)
    ));
    lower_both_and_verify(&builder, &form, &expr)?;
    Ok(())
}

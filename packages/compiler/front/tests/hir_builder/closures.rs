use ncl_hir::{CallTarget, NodeKind};
use ncl_ir::OpKind;
use ncl_object::Word;

use super::common::{
    TestResult, build, context, expand_build, fixture_text, lower_both_and_verify,
};

#[test]
fn closure_calls_capture_and_read_the_outer_lexical_binding() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(58);
    let variable = fixture.user("VALUE");
    let binding = fixture.list(&[variable, Word::fixnum(6)]);
    let bindings = fixture.list(&[binding]);
    let lambda_list = fixture.list(&[]);
    let lambda = fixture.form("LAMBDA", &[lambda_list, variable]);
    let call = fixture.form("FUNCALL", &[lambda]);
    let input = fixture.form("LET", &[bindings, call]);
    let expr = fixture.expand(input)?;
    let form = build(
        &mut builder,
        id,
        &expr,
        "(let ((value 6)) (funcall (lambda () value)))",
    )?;
    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let captures = lowered
        .entry
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|operation| match &operation.kind {
            OpKind::MakeClosure { captures, .. } => Some(captures.len()),
            _ => None,
        });

    // CLHS lexical closures retain the binding visible when the lambda is evaluated.
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_ad.htm
    assert_eq!(captures, Some(1));
    assert!(lowered.nested.iter().any(|function| {
        function.blocks.iter().any(|block| {
            block
                .ops
                .iter()
                .any(|operation| matches!(&operation.kind, OpKind::LoadCapture { index: 0 }))
        })
    }));
    Ok(())
}

#[test]
fn closure_calls_capture_a_lexical_flet_function() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(59);
    let function_name = fixture.user("LOCAL-CALLABLE");
    let definition_argument = fixture.user("VALUE");
    let definition_lambda_list = fixture.list(&[definition_argument]);
    let definition = fixture.list(&[function_name, definition_lambda_list, definition_argument]);
    let definitions = fixture.list(&[definition]);
    let lambda_list = fixture.list(&[]);
    let function_reference = fixture.form("FUNCTION", &[function_name]);
    let lambda = fixture.form("LAMBDA", &[lambda_list, function_reference]);
    let deferred_call = fixture.form("FUNCALL", &[lambda]);
    let call = fixture.form("FUNCALL", &[deferred_call, Word::fixnum(7)]);
    let input = fixture.form("FLET", &[definitions, call]);
    let source = fixture_text(
        include_str!("../fixtures/hir-bridge/flet-closure.lisp"),
        "(flet ((local-callable (value) value))\n  (funcall (funcall (lambda () #'local-callable)) 7))\n",
    );
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;
    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::FunctionReference {
            target: CallTarget::Local(_)
        }
    )));
    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let closure_capture_counts = lowered
        .entry
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter_map(|operation| match &operation.kind {
            OpKind::MakeClosure { captures, .. } => Some(captures.len()),
            _ => None,
        })
        .collect::<Vec<_>>();

    // CLHS FLET function bindings are lexical and remain visible in closures created in the body.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_flet_.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_ad.htm
    let mut closure_capture_counts = closure_capture_counts;
    closure_capture_counts.sort_unstable();
    assert_eq!(closure_capture_counts, vec![0, 1]);
    assert!(lowered.nested.iter().any(|function| {
        function.blocks.iter().any(|block| {
            block
                .ops
                .iter()
                .any(|operation| matches!(&operation.kind, OpKind::LoadCapture { index: 0 }))
        })
    }));
    Ok(())
}

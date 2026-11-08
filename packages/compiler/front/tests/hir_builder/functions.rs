use ncl_hir::{LocalFunctionKind, NodeKind, VariableTarget};
use ncl_ir::{Constant, OpKind, Terminator as MirTerminator};
use ncl_object::Word;

use super::common::{
    TestResult, build, context, expand_build, fixture_text, has_node, lower_both_and_verify,
};

#[test]
fn labels_and_lexical_assignment_lower_to_mir() -> TestResult<()> {
    let (first_id, mut builder, mut fixture) = context(42);
    let function_name = fixture.user("COUNT-DOWN");
    let argument = fixture.user("N");
    let lambda_list = fixture.list(&[argument]);
    let nil = fixture.cl("NIL");
    let recur = fixture.form("COUNT-DOWN", &[nil]);
    let branch = fixture.form("IF", &[argument, recur, Word::fixnum(7)]);
    let definition = fixture.list(&[function_name, lambda_list, branch]);
    let definitions = fixture.list(&[definition]);
    let truth = fixture.cl("T");
    let call = fixture.form("COUNT-DOWN", &[truth]);
    let input = fixture.form("LABELS", &[definitions, call]);
    let source = fixture_text(
        include_str!("../fixtures/hir-bridge/labels.lisp"),
        "(labels ((count-down (n) (if n (count-down nil) 7)))\n  (count-down t))\n",
    );
    let (expr, form) = expand_build(&mut builder, &mut fixture, first_id, input, source)?;

    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::LocalFunctions {
            kind: LocalFunctionKind::Labels,
            ..
        }
    )));
    lower_both_and_verify(&builder, &form, &expr)?;

    let id = ncl_hir::FormId::new(first_id.compilation_unit(), 1);
    let x = fixture.user("X");
    let binding = fixture.list(&[x, Word::fixnum(1)]);
    let bindings = fixture.list(&[binding]);
    let setq = fixture.form("SETQ", &[x, Word::fixnum(2)]);
    let input = fixture.form("LET", &[bindings, setq, x]);
    let source = fixture_text(
        include_str!("../fixtures/hir-bridge/assignment.lisp"),
        "(let ((x 1))\n  (setq x 2)\n  x)\n",
    );
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::Assignment {
            target: VariableTarget::Lexical(_),
            ..
        }
    )));
    lower_both_and_verify(&builder, &form, &expr)?;
    Ok(())
}

#[test]
fn labels_recursive_functions_capture_cells_for_nested_function_references() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(60);
    let function_name = fixture.user("RECURSIVE-CALLABLE");
    let argument = fixture.user("VALUE");
    let lambda_list = fixture.list(&[argument]);
    let no_parameters = fixture.list(&[]);
    let function_reference = fixture.form("FUNCTION", &[function_name]);
    let lambda = fixture.form("LAMBDA", &[no_parameters, function_reference]);
    let deferred_call = fixture.form("FUNCALL", &[lambda]);
    let definition_body = fixture.form("FUNCALL", &[deferred_call, argument]);
    let definition = fixture.list(&[function_name, lambda_list, definition_body]);
    let definitions = fixture.list(&[definition]);
    let recursive_call = fixture.form("RECURSIVE-CALLABLE", &[Word::fixnum(7)]);
    let input = fixture.form("LABELS", &[definitions, recursive_call]);
    let expr = fixture.expand(input)?;
    let form = build(
        &mut builder,
        id,
        &expr,
        "(labels ((recursive-callable (value)\n           (funcall (funcall (lambda () #'recursive-callable)) value)))\n  (recursive-callable 7))",
    )?;

    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::LocalFunctions {
            kind: LocalFunctionKind::Labels,
            ..
        }
    )));
    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;

    // CLHS LABELS definitions are visible in their own bodies and retain lexical function bindings.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_labels_.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_ad.htm
    assert!(lowered.entry.blocks.iter().any(|block| {
        block
            .ops
            .iter()
            .any(|operation| matches!(&operation.kind, OpKind::MakeValueCell { .. }))
    }));
    assert!(lowered.entry.blocks.iter().any(|block| {
        block
            .ops
            .iter()
            .any(|operation| matches!(&operation.kind, OpKind::StoreField { field: 0, .. }))
    }));
    assert!(lowered.nested.iter().any(|function| {
        let mut operations = function.blocks.iter().flat_map(|block| &block.ops);
        let captures_cell = operations
            .clone()
            .any(|operation| matches!(&operation.kind, OpKind::LoadCapture { index: 0 }));
        let loads_function = operations
            .any(|operation| matches!(&operation.kind, OpKind::LoadField { field: 0, .. }));
        captures_cell && loads_function
    }));
    Ok(())
}

#[test]
fn omitted_optional_key_and_aux_initializers_lower_to_nil() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(61);
    let optional_marker = fixture.cl("&OPTIONAL");
    let optional = fixture.user("OPTIONAL-VALUE");
    let key_marker = fixture.cl("&KEY");
    let key = fixture.user("KEY-VALUE");
    let aux_marker = fixture.cl("&AUX");
    let auxiliary = fixture.user("AUX-VALUE");
    let lambda_list = fixture.list(&[
        optional_marker,
        optional,
        key_marker,
        key,
        aux_marker,
        auxiliary,
    ]);
    let list_name = fixture.cl("LIST");
    let list_function = fixture.form("FUNCTION", &[list_name]);
    let body = fixture.form("FUNCALL", &[list_function, optional, key, auxiliary]);
    let input = fixture.form("LAMBDA", &[lambda_list, body]);
    let expr = fixture.expand(input)?;
    let form = build(
        &mut builder,
        id,
        &expr,
        "(lambda (&optional optional-value &key key-value &aux aux-value)\n  (funcall #'list optional-value key-value aux-value))",
    )?;
    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let function = lowered
        .nested
        .first()
        .ok_or_else(|| std::io::Error::other("lambda function is missing"))?;

    // CLHS lambda lists default omitted optional, keyword, and auxiliary init forms to NIL.
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_da.htm
    assert!(function.constants.contains(&Constant::Nil));
    assert!(
        function
            .blocks
            .iter()
            .any(|block| { matches!(&block.terminator, MirTerminator::Branch { .. }) })
    );
    for name in ["check-keywords", "keyword-value", "keyword-supplied-p"] {
        assert!(function.blocks.iter().any(|block| {
            block.ops.iter().any(|operation| {
                matches!(&operation.kind, OpKind::Builtin { name: actual, .. } if actual == name)
            })
        }));
    }
    assert!(function.blocks.iter().any(|block| {
        block.ops.iter().any(|operation| {
            matches!(&operation.kind, OpKind::CallClosure { args, .. } if args.len() == 4)
        })
    }));
    Ok(())
}

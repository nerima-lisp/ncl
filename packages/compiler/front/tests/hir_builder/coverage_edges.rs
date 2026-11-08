use ncl_compiler_front::hir_builder::{HirBuildError, UnsupportedHirForm};
use ncl_hir::{CallTarget, ExitTarget, NodeKind, Terminator};
use ncl_ir::{Constant, OpKind};
use ncl_object::Word;

use super::common::{
    TestResult, build, constant_for_value, context, expand_build, lower_both_and_verify,
    source_origin, verify_ir,
};

#[test]
fn direct_lambda_operator_becomes_a_value_call_to_a_closure() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(91);
    let parameter = fixture.user("VALUE");
    let lambda_list = fixture.list(&[parameter]);
    let lambda = fixture.form("LAMBDA", &[lambda_list, parameter]);
    let input = fixture.list(&[lambda, Word::fixnum(7)]);
    let source = "((lambda (value) value) 7)";
    let (_expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let (closure, argument) = form
        .nodes()
        .iter()
        .find_map(|(_, node)| match &node.kind {
            NodeKind::Call {
                target: CallTarget::Value(closure),
                arguments,
            } if arguments.len() == 1 => Some((*closure, arguments[0])),
            _ => None,
        })
        .ok_or_else(|| std::io::Error::other("direct lambda call is missing"))?;
    assert!(matches!(
        form.nodes().get(closure).map(|node| &node.kind),
        Some(NodeKind::ClosureReference { .. })
    ));
    assert!(matches!(
        form.nodes().get(argument).map(|node| &node.kind),
        Some(NodeKind::Literal(ncl_hir::Literal::Number(
            ncl_hir::NumberLiteral::Fixnum(7)
        )))
    ));
    let lowered = ncl_hir::lower::lower(&form, builder.symbols())?;
    verify_ir(&lowered.entry, "verify HIR MIR")?;
    for function in &lowered.nested {
        verify_ir(function, "verify nested HIR MIR")?;
    }
    assert_eq!(lowered.nested.len(), 1);
    assert!(lowered.entry.blocks.iter().any(|block| {
        block.ops.iter().any(|operation| {
            matches!(&operation.kind, OpKind::CallClosure { args, .. }
                if args.len() == 2
                    && constant_for_value(&lowered.entry, args[0]) == Some(&Constant::Fixnum(1))
                    && constant_for_value(&lowered.entry, args[1]) == Some(&Constant::Fixnum(7)))
        })
    }));

    // CLHS lambda forms evaluate to functions that can appear as call operators.
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_ababd.htm
    Ok(())
}

#[test]
fn function_of_lambda_becomes_a_value_function_reference() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(92);
    let parameter = fixture.user("VALUE");
    let lambda_list = fixture.list(&[parameter]);
    let lambda = fixture.form("LAMBDA", &[lambda_list, parameter]);
    let input = fixture.form("FUNCTION", &[lambda]);
    let source = "#'(lambda (value) value)";
    let expr = fixture.expand(input)?;
    let form = build(&mut builder, id, &expr, source)?;

    let closure = form
        .nodes()
        .iter()
        .find_map(|(_, node)| match &node.kind {
            NodeKind::FunctionReference {
                target: CallTarget::Value(closure),
            } => Some(*closure),
            _ => None,
        })
        .ok_or_else(|| std::io::Error::other("FUNCTION lambda reference is missing"))?;
    assert!(matches!(
        form.nodes().get(closure).map(|node| &node.kind),
        Some(NodeKind::ClosureReference { .. })
    ));
    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    assert!(lowered.entry.blocks.iter().any(|block| {
        block
            .ops
            .iter()
            .any(|operation| matches!(&operation.kind, OpKind::MakeClosure { .. }))
    }));

    // CLHS FUNCTION accepts a lambda expression and returns its function object.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_fn.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_ababd.htm
    Ok(())
}

#[test]
fn ordinary_lambda_docstring_reports_the_unrepresentable_form() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(93);
    let lambda_list = fixture.list(&[]);
    let docstring = fixture.string("documented lambda");
    let input = fixture.form("LAMBDA", &[lambda_list, docstring, Word::fixnum(7)]);
    let expr = fixture.expand(input)?;
    let (origins, origin) = source_origin(id, "(lambda () \"documented lambda\" 7)")?;

    assert!(matches!(
        builder.build_form(id, &expr, origins, origin),
        Err(HirBuildError::Unrepresentable(
            UnsupportedHirForm::Docstring
        ))
    ));
    // CLHS LAMBDA permits a documentation string in the lambda body.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_lambda.htm
    Ok(())
}

#[test]
fn flet_local_function_docstring_reports_the_unrepresentable_form() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(94);
    let name = fixture.user("LOCAL-FUNCTION");
    let lambda_list = fixture.list(&[]);
    let docstring = fixture.string("documented local function");
    let definition = fixture.list(&[name, lambda_list, docstring, Word::fixnum(7)]);
    let definitions = fixture.list(&[definition]);
    let call = fixture.form("LOCAL-FUNCTION", &[]);
    let input = fixture.form("FLET", &[definitions, call]);
    let expr = fixture.expand(input)?;
    let source = "(flet ((local-function () \"documented local function\" 7)) (local-function))";
    let (origins, origin) = source_origin(id, source)?;

    assert!(matches!(
        builder.build_form(id, &expr, origins, origin),
        Err(HirBuildError::Unrepresentable(
            UnsupportedHirForm::Docstring
        ))
    ));
    // CLHS FLET definitions allow local documentation strings.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_flet_.htm
    Ok(())
}

#[test]
fn return_from_inside_tagbody_terminates_with_its_enclosing_block_target() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(95);
    let block_name = fixture.user("DONE");
    let return_from = fixture.form("RETURN-FROM", &[block_name, Word::fixnum(23)]);
    let tagbody = fixture.form("TAGBODY", &[return_from]);
    let input = fixture.form("BLOCK", &[block_name, tagbody]);
    let source = "(block done (tagbody (return-from done 23)))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let (tagbody_id, block_id, value_id) =
        form.nodes()
            .iter()
            .find_map(|(index, node)| match &node.kind {
                NodeKind::Tagbody { .. } => {
                    let tagbody = ncl_hir::NodeId::try_from_index(id, index).ok()?;
                    let (block, value) = form.nodes().iter().find_map(|(_, child)| match &child
                        .kind
                    {
                        NodeKind::ReturnFrom {
                            target,
                            value: Some(value),
                        } => Some((*target, *value)),
                        _ => None,
                    })?;
                    Some((tagbody, block, value))
                }
                _ => None,
            })
            .ok_or_else(|| std::io::Error::other("block exit inside TAGBODY is missing"))?;
    assert!(matches!(
        form.nodes().get(block_id).map(|node| &node.kind),
        Some(NodeKind::Block { .. })
    ));
    assert!(matches!(
        form.nodes().get(value_id).map(|node| &node.kind),
        Some(NodeKind::Literal(ncl_hir::Literal::Number(
            ncl_hir::NumberLiteral::Fixnum(23)
        )))
    ));
    let flow = form
        .control_flow()
        .function(tagbody_id)
        .ok_or_else(|| std::io::Error::other("TAGBODY function flow is missing"))?;
    let entry = form
        .control_flow()
        .block(flow.entry())
        .ok_or_else(|| std::io::Error::other("TAGBODY entry block is missing"))?;
    assert!(matches!(
        entry.terminator(),
        Terminator::NonLocalExit {
            target: ExitTarget::Block(target),
            values,
            cleanups,
        } if *target == block_id
            && values.len() == 1
            && values.first() == Some(&value_id)
            && cleanups.is_empty()
    ));
    lower_both_and_verify(&builder, &form, &expr)?;

    // CLHS TAGBODY permits non-local control transfers, and RETURN-FROM exits its lexical BLOCK.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_tagbod.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_block.htm
    Ok(())
}

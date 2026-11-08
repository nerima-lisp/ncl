use ncl_compiler_front::Expr;
use ncl_compiler_front::hir_builder::{HirBuildError, UnsupportedHirForm};
use ncl_hir::{ExitTarget, NodeKind, Terminator, VariableTarget};
use ncl_ir::Constant;
use ncl_object::Word;

use super::common::{
    TestResult, build, context, expand_build, lower_both_and_verify, source_origin, verify_ir,
};

#[test]
fn zero_pair_setq_produces_nil() -> TestResult<()> {
    let (id, mut builder, _) = context(96);
    let expr = Expr::Setq(Vec::new());
    let form = build(&mut builder, id, &expr, "(setq)")?;
    let root = form
        .roots()
        .first()
        .copied()
        .ok_or_else(|| std::io::Error::other("SETQ root is missing"))?;

    assert!(matches!(
        form.nodes().get(root).map(|node| &node.kind),
        Some(NodeKind::Literal(ncl_hir::Literal::Nil))
    ));
    let (lowered, baseline) = lower_both_and_verify(&builder, &form, &expr)?;
    assert!(lowered.entry.constants.contains(&Constant::Nil));
    assert!(baseline.entry.constants.contains(&Constant::Nil));

    // CLHS SETQ returns NIL when no variable/form pairs are supplied.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_setq.htm
    Ok(())
}

#[test]
fn multiple_setq_pairs_keep_assignment_order_and_final_value() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(97);
    let first_name = fixture.user("FIRST-VALUE");
    let second_name = fixture.user("SECOND-VALUE");
    let first_binding = fixture.list(&[first_name, Word::fixnum(0)]);
    let second_binding = fixture.list(&[second_name, Word::fixnum(0)]);
    let bindings = fixture.list(&[first_binding, second_binding]);
    let setq = fixture.form(
        "SETQ",
        &[first_name, Word::fixnum(1), second_name, Word::fixnum(2)],
    );
    let input = fixture.form("LET", &[bindings, setq]);
    let source = "(let ((first-value 0) (second-value 0)) (setq first-value 1 second-value 2))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let root = form
        .roots()
        .first()
        .copied()
        .ok_or_else(|| std::io::Error::other("LET root is missing"))?;
    let root_node = form
        .nodes()
        .get(root)
        .ok_or_else(|| std::io::Error::other("LET node is missing"))?;
    let NodeKind::Let { bindings, body, .. } = &root_node.kind else {
        return Err(std::io::Error::other("root is not a LET").into());
    };
    let first_binding = bindings
        .first()
        .ok_or_else(|| std::io::Error::other("first lexical binding is missing"))?
        .binding;
    let second_binding = bindings
        .get(1)
        .ok_or_else(|| std::io::Error::other("second lexical binding is missing"))?
        .binding;
    let setq_node = body
        .first()
        .copied()
        .ok_or_else(|| std::io::Error::other("SETQ body node is missing"))?;
    let setq_node = form
        .nodes()
        .get(setq_node)
        .ok_or_else(|| std::io::Error::other("SETQ node is missing"))?;
    let NodeKind::Sequence { forms, .. } = &setq_node.kind else {
        return Err(std::io::Error::other("multiple-pair SETQ is not a sequence").into());
    };
    let first_assignment = forms
        .first()
        .copied()
        .ok_or_else(|| std::io::Error::other("first SETQ assignment is missing"))?;
    let second_assignment = forms
        .get(1)
        .copied()
        .ok_or_else(|| std::io::Error::other("second SETQ assignment is missing"))?;
    assert_eq!(forms.len(), 2);

    let first_value = match form.nodes().get(first_assignment).map(|node| &node.kind) {
        Some(NodeKind::Assignment {
            target: VariableTarget::Lexical(target),
            value,
        }) if *target == first_binding => *value,
        _ => return Err(std::io::Error::other("first SETQ target is not first-value").into()),
    };
    let second_value = match form.nodes().get(second_assignment).map(|node| &node.kind) {
        Some(NodeKind::Assignment {
            target: VariableTarget::Lexical(target),
            value,
        }) if *target == second_binding => *value,
        _ => return Err(std::io::Error::other("second SETQ target is not second-value").into()),
    };
    assert!(matches!(
        form.nodes().get(first_value).map(|node| &node.kind),
        Some(NodeKind::Literal(ncl_hir::Literal::Number(
            ncl_hir::NumberLiteral::Fixnum(1)
        )))
    ));
    assert!(matches!(
        form.nodes().get(second_value).map(|node| &node.kind),
        Some(NodeKind::Literal(ncl_hir::Literal::Number(
            ncl_hir::NumberLiteral::Fixnum(2)
        )))
    ));
    lower_both_and_verify(&builder, &form, &expr)?;

    // CLHS SETQ evaluates assignment pairs from left to right and returns the last form's value.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_setq.htm
    Ok(())
}

#[test]
fn single_pair_setq_is_the_assignment_expression() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(100);
    let value_name = fixture.user("VALUE");
    let binding = fixture.list(&[value_name, Word::fixnum(0)]);
    let bindings = fixture.list(&[binding]);
    let setq = fixture.form("SETQ", &[value_name, Word::fixnum(11)]);
    let input = fixture.form("LET", &[bindings, setq]);
    let source = "(let ((value 0)) (setq value 11))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let root = form
        .roots()
        .first()
        .copied()
        .ok_or_else(|| std::io::Error::other("LET root is missing"))?;
    let NodeKind::Let { bindings, body, .. } = &form
        .nodes()
        .get(root)
        .ok_or_else(|| std::io::Error::other("LET node is missing"))?
        .kind
    else {
        return Err(std::io::Error::other("root is not a LET").into());
    };
    let binding = bindings
        .first()
        .ok_or_else(|| std::io::Error::other("lexical binding is missing"))?
        .binding;
    let assignment = body
        .first()
        .copied()
        .ok_or_else(|| std::io::Error::other("SETQ body is missing"))?;
    let value = match form.nodes().get(assignment).map(|node| &node.kind) {
        Some(NodeKind::Assignment {
            target: VariableTarget::Lexical(target),
            value,
        }) if *target == binding => *value,
        _ => return Err(std::io::Error::other("single-pair SETQ is not its assignment").into()),
    };
    assert!(matches!(
        form.nodes().get(value).map(|node| &node.kind),
        Some(NodeKind::Literal(ncl_hir::Literal::Number(
            ncl_hir::NumberLiteral::Fixnum(11)
        )))
    ));
    lower_both_and_verify(&builder, &form, &expr)?;

    // CLHS SETQ with one pair assigns the value form to its variable.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_setq.htm
    Ok(())
}

#[test]
fn repeated_tag_name_in_one_tagbody_fails_with_a_typed_unsupported_form() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(98);
    let repeated_tag = fixture.user("REPEAT");
    let input = fixture.form("TAGBODY", &[repeated_tag, repeated_tag]);
    let expr = fixture.expand(input)?;
    let (origins, origin) = source_origin(id, "(tagbody repeat repeat)")?;

    assert!(matches!(
        builder.build_form(id, &expr, origins, origin),
        Err(HirBuildError::Unrepresentable(
            UnsupportedHirForm::DuplicateTag
        ))
    ));
    // CLHS TAGBODY establishes lexical tags; this bridge rejects a repeated tag identity explicitly.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_tagbod.htm
    Ok(())
}

#[test]
fn nested_tagbodies_resolve_go_to_the_innermost_matching_tag() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(101);
    let repeated_tag = fixture.user("REPEATED-TAG");
    let go = fixture.form("GO", &[repeated_tag]);
    let done = fixture.user("DONE");
    let inner = fixture.form("TAGBODY", &[repeated_tag, go, done]);
    let outer = fixture.form("TAGBODY", &[repeated_tag, inner]);
    let source = "(tagbody repeated-tag (tagbody repeated-tag (go repeated-tag) done))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, outer, source)?;

    let (outer_id, outer_tag, inner_id, inner_tag) = form
        .nodes()
        .iter()
        .find_map(|(index, node)| {
            let NodeKind::Tagbody {
                tags: outer_tags,
                forms,
            } = &node.kind
            else {
                return None;
            };
            let inner_id = forms.iter().find_map(|form_id| {
                matches!(
                    form.nodes().get(*form_id).map(|node| &node.kind),
                    Some(NodeKind::Tagbody { .. })
                )
                .then_some(*form_id)
            })?;
            let NodeKind::Tagbody {
                tags: inner_tags, ..
            } = &form.nodes().get(inner_id)?.kind
            else {
                return None;
            };
            Some((
                ncl_hir::NodeId::try_from_index(id, index).ok()?,
                *outer_tags.first()?,
                inner_id,
                *inner_tags.first()?,
            ))
        })
        .ok_or_else(|| std::io::Error::other("nested TAGBODY tags are missing"))?;
    assert_eq!(outer_tag.name, inner_tag.name);
    assert_ne!(outer_tag.block, inner_tag.block);
    assert_ne!(outer_id, inner_id);

    let go_node = form
        .nodes()
        .iter()
        .find_map(|(_, node)| match &node.kind {
            NodeKind::Go { target } => Some(*target),
            _ => None,
        })
        .ok_or_else(|| std::io::Error::other("GO node is missing"))?;
    assert_eq!(go_node, inner_tag.block);
    let flow = form
        .control_flow()
        .function(inner_id)
        .ok_or_else(|| std::io::Error::other("inner TAGBODY flow is missing"))?;
    let entry = form
        .control_flow()
        .block(flow.entry())
        .ok_or_else(|| std::io::Error::other("inner TAGBODY entry is missing"))?;
    assert!(matches!(
        entry.terminator(),
        Terminator::Jump(target) if *target == inner_tag.block
    ));
    lower_both_and_verify(&builder, &form, &expr)?;

    // CLHS TAGBODY tags are lexical, and GO transfers to the nearest enclosing matching tag.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_tagbod.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_go.htm
    Ok(())
}

#[test]
fn throw_inside_tagbody_terminates_its_entry_with_the_matching_catch_target() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(99);
    let tag = fixture.keyword("ESCAPE");
    let quoted_tag = fixture.form("QUOTE", &[tag]);
    let throw = fixture.form("THROW", &[quoted_tag, Word::fixnum(29)]);
    let tagbody = fixture.form("TAGBODY", &[throw]);
    let input = fixture.form("CATCH", &[quoted_tag, tagbody]);
    let source = "(catch :escape (tagbody (throw :escape 29)))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let tagbody_id = form
        .nodes()
        .iter()
        .find_map(|(index, node)| match &node.kind {
            NodeKind::Tagbody { .. } => ncl_hir::NodeId::try_from_index(id, index).ok(),
            _ => None,
        })
        .ok_or_else(|| std::io::Error::other("TAGBODY node is missing"))?;
    let (tag_node, value_node) = form
        .nodes()
        .iter()
        .find_map(|(_, node)| match &node.kind {
            NodeKind::Throw { tag, value } => Some((*tag, *value)),
            _ => None,
        })
        .ok_or_else(|| std::io::Error::other("THROW node is missing"))?;
    assert!(matches!(
        form.nodes().get(value_node).map(|node| &node.kind),
        Some(NodeKind::Literal(ncl_hir::Literal::Number(
            ncl_hir::NumberLiteral::Fixnum(29)
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
            target: ExitTarget::Dynamic(target),
            values,
            cleanups,
        } if *target == tag_node
            && values.len() == 1
            && values.first() == Some(&value_node)
            && cleanups.is_empty()
    ));
    lower_both_and_verify(&builder, &form, &expr)?;

    // CLHS THROW transfers control to the matching active CATCH after evaluating its value.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_throw.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_catch.htm
    Ok(())
}

#[test]
fn throw_inside_unwind_protected_tagbody_preserves_its_cleanup_exit() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(102);
    let tag = fixture.keyword("ESCAPE");
    let quoted_tag = fixture.form("QUOTE", &[tag]);
    let throw = fixture.form("THROW", &[quoted_tag, Word::fixnum(31)]);
    let tagbody = fixture.form("TAGBODY", &[throw]);
    let input = fixture.form("UNWIND-PROTECT", &[tagbody, Word::fixnum(47)]);
    let source = "(unwind-protect (tagbody (throw :escape 31)) 47)";
    let (_expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let tagbody_id = form
        .nodes()
        .iter()
        .find_map(|(index, node)| {
            matches!(&node.kind, NodeKind::Tagbody { .. })
                .then(|| ncl_hir::NodeId::try_from_index(id, index).ok())
                .flatten()
        })
        .ok_or_else(|| std::io::Error::other("TAGBODY node is missing"))?;
    let cleanup = form
        .nodes()
        .iter()
        .find_map(|(_, node)| match &node.kind {
            NodeKind::UnwindProtect { cleanup, .. } => Some(*cleanup),
            _ => None,
        })
        .ok_or_else(|| std::io::Error::other("UNWIND-PROTECT cleanup is missing"))?;
    let (tag_node, value_node) = form
        .nodes()
        .iter()
        .find_map(|(_, node)| match &node.kind {
            NodeKind::NonLocalExit {
                target: ExitTarget::Dynamic(tag),
                values,
                cleanups,
            } if cleanups.len() == 1 && cleanups.first() == Some(&cleanup) => {
                values.first().map(|value| (*tag, *value))
            }
            _ => None,
        })
        .ok_or_else(|| std::io::Error::other("cleanup-preserving THROW exit is missing"))?;
    assert!(matches!(
        form.nodes().get(value_node).map(|node| &node.kind),
        Some(NodeKind::Literal(ncl_hir::Literal::Number(
            ncl_hir::NumberLiteral::Fixnum(31)
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
            target: ExitTarget::Dynamic(target),
            values,
            cleanups,
        } if *target == tag_node
            && values.len() == 1
            && values.first() == Some(&value_node)
            && cleanups.len() == 1
            && cleanups.first() == Some(&cleanup)
    ));
    let hir_lowered = ncl_hir::lower::lower(&form, builder.symbols())?;
    verify_ir(
        &hir_lowered.entry,
        "verify HIR cleanup-preserving THROW MIR",
    )?;
    for function in &hir_lowered.nested {
        verify_ir(function, "verify nested HIR cleanup-preserving THROW MIR")?;
    }

    // CLHS UNWIND-PROTECT runs cleanup forms when THROW exits the protected form.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_unwind.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_throw.htm
    Ok(())
}

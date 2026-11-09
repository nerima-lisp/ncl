use ncl_hir::NodeKind;
use ncl_ir::{Constant, HandlerKind, OpKind, Terminator as MirTerminator};
use ncl_object::Word;

use super::common::{TestResult, constant_for_value, context, expand_build, lower_both_and_verify};
use super::control::{handler_region, named_call_position};

#[test]
fn empty_tagbody_returns_nil_and_block_returns_its_body_value() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(116);
    let empty_tagbody = fixture.form("TAGBODY", &[]);
    let block_name = fixture.user("RESULT");
    let block = fixture.form("BLOCK", &[block_name, Word::fixnum(59)]);
    let input = fixture.form("PROGN", &[empty_tagbody, block]);
    let source = "(progn (tagbody) (block result 59))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::Tagbody { tags, forms } if tags.is_empty() && forms.is_empty()
    )));
    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::Block { body, .. } if body.len() == 1
    )));

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    assert!(lowered.entry.constants.contains(&Constant::Nil));
    assert!(lowered.entry.constants.contains(&Constant::Fixnum(59)));

    // CLHS TAGBODY returns NIL; BLOCK returns the values of its body unless RETURN-FROM exits it.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_tagbod.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_block.htm
    Ok(())
}

#[test]
fn empty_catch_and_progv_bodies_and_valueless_return_from_produce_nil() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(117);
    let done = fixture.user("DONE");
    let return_from = fixture.form("RETURN-FROM", &[done]);
    let block = fixture.form("BLOCK", &[done, return_from]);
    let tag = fixture.keyword("TAG");
    let catch = fixture.form("CATCH", &[tag]);
    let nil = fixture.cl("NIL");
    let progv = fixture.form("PROGV", &[nil, nil]);
    let input = fixture.form("PROGN", &[block, catch, progv]);
    let source = "(progn (block done (return-from done)) (catch :tag) (progv nil nil))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(
        form.nodes()
            .iter()
            .any(|(_, node)| matches!(&node.kind, NodeKind::ReturnFrom { value: None, .. }))
    );
    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::Catch { body, .. } if body.is_empty()
    )));
    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::DynamicBinding { body, .. } if body.is_empty()
    )));

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    assert!(lowered.entry.constants.contains(&Constant::Nil));

    // CLHS RETURN-FROM without a result form, empty CATCH, and empty PROGV bodies return NIL.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_block.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_catch.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_progv.htm
    Ok(())
}

#[test]
fn nested_unwind_protect_runs_each_cleanup_while_throw_unwinds_outward() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(122);
    let tag = fixture.keyword("ESCAPE");
    let throw = fixture.form("THROW", &[tag, Word::fixnum(43)]);
    let inner_name = fixture.user("INNER-CLEANUP");
    let inner_cleanup = fixture.list(&[inner_name]);
    let inner = fixture.form("UNWIND-PROTECT", &[throw, inner_cleanup]);
    let outer_name = fixture.user("OUTER-CLEANUP");
    let outer_cleanup = fixture.list(&[outer_name]);
    let outer = fixture.form("UNWIND-PROTECT", &[inner, outer_cleanup]);
    let input = fixture.form("CATCH", &[tag, outer]);
    let source = "(catch :escape (unwind-protect (unwind-protect (throw :escape 43) (inner-cleanup)) (outer-cleanup)))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let unwind_regions = lowered
        .entry
        .handler_regions
        .iter()
        .filter(|region| region.kind == HandlerKind::UnwindProtect)
        .collect::<Vec<_>>();
    assert_eq!(unwind_regions.len(), 2);
    let throw_block = lowered
        .entry
        .blocks
        .iter()
        .find(|block| {
            matches!(&block.terminator, MirTerminator::Throw { .. })
                && block.ops.iter().any(|operation| {
                    matches!(
                        &operation.kind,
                        OpKind::Builtin { name, args, .. } if name == "throw" && args.len() == 2
                    )
                })
        })
        .ok_or_else(|| std::io::Error::other("THROW block is missing"))?;
    let inner_block =
        named_call_position(&lowered.entry, "COMMON-LISP-USER", "INNER-CLEANUP", true)
            .map(|(block, _)| block)
            .ok_or_else(|| std::io::Error::other("inner cleanup call is missing"))?;
    let outer_block =
        named_call_position(&lowered.entry, "COMMON-LISP-USER", "OUTER-CLEANUP", true)
            .map(|(block, _)| block)
            .ok_or_else(|| std::io::Error::other("outer cleanup call is missing"))?;
    let inner_region = unwind_regions
        .iter()
        .find(|region| region.cleanup == Some(inner_block))
        .ok_or_else(|| std::io::Error::other("inner cleanup handler is missing"))?;
    let outer_region = unwind_regions
        .iter()
        .find(|region| region.cleanup == Some(outer_block))
        .ok_or_else(|| std::io::Error::other("outer cleanup handler is missing"))?;
    assert!(inner_region.protected.contains(&throw_block.id));
    assert!(outer_region.protected.contains(&throw_block.id));
    assert!(outer_region.protected.contains(&inner_region.handler));
    assert!(lowered.entry.handler_regions.iter().any(|region| {
        region.kind == HandlerKind::Catch && region.protected.contains(&throw_block.id)
    }));

    // CLHS runs each UNWIND-PROTECT cleanup before THROW leaves that form's dynamic extent.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_unwind.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_throw.htm
    Ok(())
}

#[test]
fn go_through_unwind_protect_runs_cleanup_before_tagbody_transfer() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(129);
    let start = fixture.user("START");
    let done = fixture.user("DONE");
    let go = fixture.form("GO", &[done]);
    let cleanup_name = fixture.user("CLEANUP");
    let cleanup = fixture.list(&[cleanup_name]);
    let unwind = fixture.form("UNWIND-PROTECT", &[go, cleanup]);
    let input = fixture.form("TAGBODY", &[start, unwind, done]);
    let source = "(tagbody start (unwind-protect (go done) (cleanup)) done)";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::NonLocalExit {
            target: ncl_hir::ExitTarget::Tag(_),
            cleanups,
            ..
        } if cleanups.len() == 1
    )));

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let cleanup_block = named_call_position(&lowered.entry, "COMMON-LISP-USER", "CLEANUP", true)
        .map(|(block, _)| block)
        .ok_or_else(|| std::io::Error::other("cleanup call is missing"))?;
    assert!(
        lowered
            .entry
            .blocks
            .iter()
            .any(|block| block.id == cleanup_block)
    );
    assert!(
        lowered.entry.handler_regions.iter().any(|region| {
            region.kind == HandlerKind::UnwindProtect && region.cleanup.is_some()
        })
    );
    assert!(
        lowered
            .entry
            .blocks
            .iter()
            .any(|block| matches!(&block.terminator, MirTerminator::Jump { .. }))
    );

    // CLHS GO transfers control to its tag after active UNWIND-PROTECT cleanup forms run.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_go.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_unwind.htm
    Ok(())
}

#[test]
fn cleanup_throw_replaces_return_from() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(130);
    let tag = fixture.keyword("CLEANUP-EXIT");
    let done = fixture.user("DONE");
    let return_from = fixture.form("RETURN-FROM", &[done, Word::fixnum(9)]);
    let throw = fixture.form("THROW", &[tag, Word::fixnum(17)]);
    let later_cleanup_name = fixture.user("AFTER-CLEANUP");
    let later_cleanup = fixture.list(&[later_cleanup_name]);
    let unwind = fixture.form("UNWIND-PROTECT", &[return_from, throw, later_cleanup]);
    let block = fixture.form("BLOCK", &[done, unwind]);
    let input = fixture.form("CATCH", &[tag, block]);
    let source = "(catch :cleanup-exit (block done (unwind-protect (return-from done 9) (throw :cleanup-exit 17) (after-cleanup))))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::NonLocalExit {
            target: ncl_hir::ExitTarget::Block(_),
            values,
            cleanups,
        } if values.len() == 1 && cleanups.len() == 1
    )));
    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::UnwindProtect { cleanup_forms, .. } if cleanup_forms.len() == 2
    )));

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    assert!(lowered.entry.constants.contains(&Constant::Fixnum(17)));
    assert!(lowered.entry.blocks.iter().any(|block| {
        matches!(&block.terminator, MirTerminator::Throw { .. })
            && block.ops.iter().any(|operation| {
                matches!(
                    &operation.kind,
                    OpKind::Builtin { name, args, .. } if name == "throw" && args.len() == 2
                )
            })
    }));
    assert!(
        lowered
            .entry
            .handler_regions
            .iter()
            .any(|region| { region.kind == HandlerKind::Catch })
    );

    // CLHS UNWIND-PROTECT cleanup forms run before an exit; a transfer from cleanup supersedes it.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_unwind.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_throw.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_block.htm
    Ok(())
}

#[test]
fn cleanup_return_from_supersedes_protected_value() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(138);
    let done = fixture.user("DONE");
    let return_from = fixture.form("RETURN-FROM", &[done, Word::fixnum(2)]);
    let protected = fixture.form("UNWIND-PROTECT", &[Word::fixnum(1), return_from]);
    let input = fixture.form("BLOCK", &[done, protected]);
    let source = "(block done (unwind-protect 1 (return-from done 2)))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::UnwindProtect { cleanup_forms, .. } if cleanup_forms.iter().any(|id| matches!(
            form.nodes().get(*id).map(|node| &node.kind),
            Some(NodeKind::ReturnFrom { .. })
        ))
    )));

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let region = handler_region(&lowered.entry, HandlerKind::UnwindProtect)?;
    let block_exit = named_call_position(&lowered.entry, "COMMON-LISP", "VALUES-LIST", true)
        .map(|(block, _)| block)
        .ok_or_else(|| std::io::Error::other("block result is missing"))?;
    assert!(lowered.entry.blocks.iter().any(|exit| {
        region.protected.contains(&exit.id)
            && exit.ops.iter().any(|op| {
                matches!(&op.kind, OpKind::LeaveHandler { region: id } if *id == region.id)
            })
            && matches!(&exit.terminator, MirTerminator::Jump { target, args } if {
                let carries_cleanup_value = args.first().is_some_and(|transfer_value| {
                    exit.ops.iter().any(|op| {
                        let OpKind::CallClosure { args, .. } = &op.kind else { return false; };
                        args.get(1).and_then(|value| constant_for_value(&lowered.entry, *value))
                            == Some(&Constant::Fixnum(2))
                            && op.results.first().is_some_and(|(result, _)| result == transfer_value)
                    })
                });
                carries_cleanup_value && *target == block_exit
            })
    }));

    // CLHS says UNWIND-PROTECT cleanup runs before RETURN-FROM, whose transfer then takes effect.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_unwind.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_block.htm
    Ok(())
}

#[test]
fn cleanup_throw_supersedes_pending_throw() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(139);
    let tag = fixture.keyword("TAG");
    let pending = fixture.form("THROW", &[tag, Word::fixnum(1)]);
    let cleanup_throw = fixture.form("THROW", &[tag, Word::fixnum(2)]);
    let unwind = fixture.form("UNWIND-PROTECT", &[pending, cleanup_throw]);
    let input = fixture.form("CATCH", &[tag, unwind]);
    let source = "(catch :tag (unwind-protect (throw :tag 1) (throw :tag 2)))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::UnwindProtect { protected, cleanup_forms, .. }
            if matches!(form.nodes().get(*protected).map(|node| &node.kind),
                Some(NodeKind::NonLocalExit { target: ncl_hir::ExitTarget::Dynamic(_), values, cleanups })
                    if values.len() == 1 && cleanups.len() == 1)
                && cleanup_forms.iter().any(|id| matches!(form.nodes().get(*id).map(|node| &node.kind),
                    Some(NodeKind::Throw { .. })))
    )));

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let throws = lowered
        .entry
        .blocks
        .iter()
        .filter_map(|block| {
            let value = block.ops.iter().find_map(|op| match &op.kind {
                OpKind::Builtin { name, args, .. } if name == "throw" => args
                    .get(1)
                    .and_then(|value| constant_for_value(&lowered.entry, *value))
                    .and_then(|value| match value {
                        Constant::Fixnum(value) => Some(*value),
                        _ => None,
                    }),
                _ => None,
            })?;
            matches!(&block.terminator, MirTerminator::Throw { .. }).then_some((block, value))
        })
        .collect::<Vec<_>>();
    let pending = throws
        .iter()
        .find(|(_, value)| *value == 1)
        .map(|(block, _)| *block)
        .ok_or_else(|| std::io::Error::other("protected THROW is missing"))?;
    let cleanup = throws
        .iter()
        .find(|(_, value)| *value == 2)
        .map(|(block, _)| *block)
        .ok_or_else(|| std::io::Error::other("cleanup THROW is missing"))?;
    let unwind = handler_region(&lowered.entry, HandlerKind::UnwindProtect)?;
    let catch = handler_region(&lowered.entry, HandlerKind::Catch)?;
    assert!(unwind.protected.contains(&pending.id));
    assert_eq!(unwind.cleanup, Some(cleanup.id));
    assert!(catch.protected.contains(&cleanup.id));

    // CLHS cleanup forms run during UNWIND-PROTECT exit; THROW from cleanup takes effect.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_unwind.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_throw.htm
    Ok(())
}

#[test]
fn backward_go_through_unwind_protect_runs_cleanup_before_looping() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(140);
    let start = fixture.user("START");
    let before_name = fixture.user("BEFORE");
    let before = fixture.list(&[before_name]);
    let go = fixture.form("GO", &[start]);
    let cleanup_name = fixture.user("CLEANUP");
    let cleanup = fixture.list(&[cleanup_name]);
    let unwind = fixture.form("UNWIND-PROTECT", &[go, cleanup]);
    let input = fixture.form("TAGBODY", &[start, before, unwind]);
    let source = "(tagbody start (before) (unwind-protect (go start) (cleanup)))";
    let (_expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::NonLocalExit { target: ncl_hir::ExitTarget::Tag(_), values, cleanups }
            if values.is_empty() && cleanups.len() == 1
    )));
    // The legacy frontend lowerer emits invalid MIR for this backedge; inspect the HIR path directly.
    let lowered = ncl_hir::lower::lower(&form, builder.symbols())?;
    let (start_block, _) = named_call_position(&lowered.entry, "COMMON-LISP-USER", "BEFORE", true)
        .ok_or_else(|| std::io::Error::other("backward GO target form is missing"))?;
    let (cleanup_block, cleanup_index) =
        named_call_position(&lowered.entry, "COMMON-LISP-USER", "CLEANUP", true)
            .ok_or_else(|| std::io::Error::other("cleanup call is missing"))?;
    let region = handler_region(&lowered.entry, HandlerKind::UnwindProtect)?;
    let transfer = lowered
        .entry
        .blocks
        .iter()
        .find(|block| block.id == cleanup_block)
        .ok_or_else(|| std::io::Error::other("cleanup transfer block is missing"))?;
    let leave_index = transfer
        .ops
        .iter()
        .position(|op| matches!(&op.kind, OpKind::LeaveHandler { region: id } if *id == region.id))
        .ok_or_else(|| std::io::Error::other("cleanup transfer does not leave its handler"))?;
    assert!(cleanup_index < leave_index);
    assert_eq!(transfer.ops.len(), leave_index + 2);
    assert!(matches!(
        transfer.ops.last().map(|op| &op.kind),
        Some(OpKind::Safepoint)
    ));
    assert!(
        matches!(&transfer.terminator, MirTerminator::Jump { target, .. } if *target == start_block)
    );

    // CLHS GO transfers to an earlier tag after active UNWIND-PROTECT cleanup completes.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_go.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_unwind.htm
    Ok(())
}

#[test]
fn non_local_exits_during_catch_tag_and_progv_arguments_use_outer_catchers() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(136);
    let escape = fixture.keyword("ESCAPE");

    let catch_tag_exit = fixture.form("THROW", &[escape, Word::fixnum(41)]);
    let inner_catch = fixture.form("CATCH", &[catch_tag_exit, Word::fixnum(9)]);
    let catch_tag_case = fixture.form("CATCH", &[escape, inner_catch]);

    let progv_symbols_exit = fixture.form("THROW", &[escape, Word::fixnum(42)]);
    let nil = fixture.cl("NIL");
    let progv_symbols_case = fixture.form("PROGV", &[progv_symbols_exit, nil, Word::fixnum(13)]);
    let progv_symbols_case = fixture.form("CATCH", &[escape, progv_symbols_case]);

    let nil = fixture.cl("NIL");
    let progv_values_exit = fixture.form("THROW", &[escape, Word::fixnum(43)]);
    let progv_values_case = fixture.form("PROGV", &[nil, progv_values_exit, Word::fixnum(17)]);
    let progv_values_case = fixture.form("CATCH", &[escape, progv_values_case]);

    let input = fixture.form(
        "PROGN",
        &[catch_tag_case, progv_symbols_case, progv_values_case],
    );
    let source = "(progn (catch :escape (catch (throw :escape 41) 9)) (catch :escape (progv (throw :escape 42) nil 13)) (catch :escape (progv nil (throw :escape 43) 17)))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert_eq!(
        form.nodes()
            .iter()
            .filter(|(_, node)| matches!(&node.kind, NodeKind::Catch { .. }))
            .count(),
        4
    );
    assert_eq!(
        form.nodes()
            .iter()
            .filter(|(_, node)| matches!(&node.kind, NodeKind::DynamicBinding { .. }))
            .count(),
        2
    );

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let throw_blocks = lowered
        .entry
        .blocks
        .iter()
        .filter(|block| matches!(&block.terminator, MirTerminator::Throw { .. }))
        .collect::<Vec<_>>();
    assert_eq!(throw_blocks.len(), 3);
    let catch_regions = lowered
        .entry
        .handler_regions
        .iter()
        .filter(|region| region.kind == HandlerKind::Catch)
        .collect::<Vec<_>>();
    assert_eq!(catch_regions.len(), 3);
    assert!(
        lowered
            .entry
            .handler_regions
            .iter()
            .all(|region| region.kind != HandlerKind::Progv)
    );
    for block in &throw_blocks {
        assert_eq!(
            catch_regions
                .iter()
                .filter(|region| region.protected.contains(&block.id))
                .count(),
            1,
            "each argument-evaluation THROW must be protected only by its outer CATCH"
        );
    }
    for region in &catch_regions {
        assert_eq!(
            throw_blocks
                .iter()
                .filter(|block| region.protected.contains(&block.id))
                .count(),
            1,
            "each outer CATCH must protect only its argument-evaluation THROW"
        );
    }

    // CLHS evaluates a CATCH tag before establishing its catcher and evaluates PROGV's
    // symbols and values arguments before establishing its dynamic bindings.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_catch.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_progv.htm
    Ok(())
}

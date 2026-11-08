use super::common::{TestResult, context, has_node, verify_ir};
use super::control::{emits_named_call, handler_normal_result, handler_region, named_call_block};
use ncl_hir::NodeKind;
use ncl_ir::{Constant, OpKind};
use ncl_object::Word;

#[test]
fn return_from_runs_unwind_cleanup_and_preserves_multiple_values() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(36);
    let done = fixture.user("DONE");
    let values = fixture.form("VALUES", &[Word::fixnum(1), Word::fixnum(2)]);
    let preserved = fixture.form("MULTIPLE-VALUE-PROG1", &[values, Word::fixnum(3)]);
    let exit = fixture.form("RETURN-FROM", &[done, preserved]);
    let cleanup_name = fixture.user("CLEANUP");
    let cleanup = fixture.list(&[cleanup_name]);
    let protected = fixture.form("UNWIND-PROTECT", &[exit, cleanup]);
    let input = fixture.form("BLOCK", &[done, protected]);
    let source = "(block done\n  (unwind-protect\n      (return-from done (multiple-value-prog1 (values 1 2) 3))\n    (cleanup)))\n";
    let expr = fixture.expand(input)?;
    let form = super::common::build(&mut builder, id, &expr, source)?;
    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::NonLocalExit { target: ncl_hir::ExitTarget::Block(_), cleanups, .. } if cleanups.len() == 1
    )));
    let hir = ncl_hir::lower::lower(&form, builder.symbols())?;
    let baseline = ncl_compiler_front::lower::lower_toplevel(&expr)?;
    verify_ir(&hir.entry, "verify HIR MIR")?;
    verify_ir(&baseline.entry, "verify baseline MIR")?;
    assert!(emits_named_call(
        &hir.entry,
        "NCL-EXT",
        "CAPTURE-MULTIPLE-VALUES",
        false,
    ));
    assert!(emits_named_call(
        &hir.entry,
        "COMMON-LISP",
        "VALUES-LIST",
        true,
    ));
    // CLHS requires cleanup forms to run before RETURN-FROM transfers control and values.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_unwind.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_block.htm
    let region = handler_region(&hir.entry, ncl_ir::HandlerKind::UnwindProtect)?;
    assert!(region.cleanup.is_some());
    let cleanup_block_id = named_call_block(&hir.entry, "COMMON-LISP-USER", "CLEANUP", true)
        .ok_or_else(|| std::io::Error::other("cleanup call is missing"))?;
    let cleanup_block = hir
        .entry
        .blocks
        .iter()
        .find(|block| block.id == cleanup_block_id)
        .ok_or_else(|| std::io::Error::other("cleanup block is missing"))?;
    assert!(region.protected.contains(&cleanup_block_id));
    assert!(
        cleanup_block.ops.iter().any(|op| {
            matches!(&op.kind, OpKind::LeaveHandler { region: id } if *id == region.id)
        })
    );
    let ncl_ir::Terminator::Jump { target, args } = &cleanup_block.terminator else {
        return Err(std::io::Error::other("RETURN-FROM jump after cleanup is missing").into());
    };
    assert_eq!(args.len(), 1);
    let return_block = hir
        .entry
        .blocks
        .iter()
        .find(|block| block.id == *target)
        .ok_or_else(|| std::io::Error::other("RETURN-FROM target block is missing"))?;
    assert_eq!(args.len(), return_block.params.len());
    assert_eq!(
        named_call_block(&hir.entry, "COMMON-LISP", "VALUES-LIST", true),
        Some(return_block.id)
    );
    let returned_values = return_block
        .params
        .first()
        .ok_or_else(|| std::io::Error::other("RETURN-FROM value parameter is missing"))?;
    assert!(return_block.ops.iter().any(|op| {
        matches!(&op.kind, OpKind::CallClosure { args, .. } if args.get(1) == Some(&returned_values.value))
    }));
    Ok(())
}

#[test]
fn catch_normal_completion_returns_body_value_and_leaves_handler() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(58);
    let tag = fixture.keyword("TAG");
    let input = fixture.form("CATCH", &[tag, Word::fixnum(7)]);
    let expr = fixture.expand(input)?;
    let form = super::common::build(&mut builder, id, &expr, "(catch :tag 7)")?;
    let lowered = ncl_hir::lower::lower(&form, builder.symbols())?;
    verify_ir(&lowered.entry, "verify normal catch MIR")?;

    // CLHS specifies that normal CATCH completion returns its body values and disestablishes the tag.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_catch.htm
    let region = handler_region(&lowered.entry, ncl_ir::HandlerKind::Catch)?;
    assert!(region.catch_tag.is_some());
    assert!(lowered.entry.constants.contains(&Constant::Fixnum(7)));
    assert!(handler_normal_result(&lowered.entry, region));
    Ok(())
}

#[test]
fn progv_normal_completion_returns_body_value_and_restores_bindings() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(59);
    let special = fixture.user("SPECIAL");
    let symbol_list = fixture.list(&[special]);
    let quoted_symbols = fixture.form("QUOTE", &[symbol_list]);
    let value_list = fixture.list(&[Word::fixnum(5)]);
    let quoted_values = fixture.form("QUOTE", &[value_list]);
    let input = fixture.form("PROGV", &[quoted_symbols, quoted_values, Word::fixnum(9)]);
    let expr = fixture.expand(input)?;
    let form = super::common::build(&mut builder, id, &expr, "(progv '(special) '(5) 9)")?;
    let lowered = ncl_hir::lower::lower(&form, builder.symbols())?;
    verify_ir(&lowered.entry, "verify normal progv MIR")?;

    // CLHS specifies that PROGV undoes its dynamic bindings when its forms finish normally.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_progv.htm
    let region = handler_region(&lowered.entry, ncl_ir::HandlerKind::Progv)?;
    assert_eq!(region.binding_targets.len(), 2);
    assert!(lowered.entry.constants.contains(&Constant::Fixnum(9)));
    assert!(handler_normal_result(&lowered.entry, region));
    Ok(())
}

#[test]
fn progv_return_from_leaves_handler_and_transfers_body_value() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(60);
    let done = fixture.user("DONE");
    let exit = fixture.form("RETURN-FROM", &[done, Word::fixnum(9)]);
    let special = fixture.user("SPECIAL");
    let symbol_list = fixture.list(&[special]);
    let quoted_symbols = fixture.form("QUOTE", &[symbol_list]);
    let value_list = fixture.list(&[Word::fixnum(5)]);
    let quoted_values = fixture.form("QUOTE", &[value_list]);
    let progv = fixture.form("PROGV", &[quoted_symbols, quoted_values, exit]);
    let input = fixture.form("BLOCK", &[done, progv]);
    let expr = fixture.expand(input)?;
    let form = super::common::build(
        &mut builder,
        id,
        &expr,
        "(block done (progv '(special) '(5) (return-from done 9)))",
    )?;
    let lowered = ncl_hir::lower::lower(&form, builder.symbols())?;
    verify_ir(&lowered.entry, "verify progv return-from MIR")?;

    // CLHS specifies that PROGV bindings are undone on exit and RETURN-FROM transfers its values.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_progv.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_block.htm
    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::ReturnFrom { .. }
    )));
    let region = handler_region(&lowered.entry, ncl_ir::HandlerKind::Progv)?;
    assert_eq!(region.binding_targets.len(), 2);
    let return_block = lowered
        .entry
        .blocks
        .iter()
        .find(|block| {
            region.protected.contains(&block.id)
                && block.ops.iter().any(|op| {
                    matches!(&op.kind, OpKind::LeaveHandler { region: id } if *id == region.id)
                })
                && matches!(&block.terminator, ncl_ir::Terminator::Jump { args, .. } if args.len() == 1)
        })
        .ok_or_else(|| std::io::Error::other("RETURN-FROM jump after PROGV is missing"))?;
    let ncl_ir::Terminator::Jump { target, args } = &return_block.terminator else {
        return Err(std::io::Error::other("RETURN-FROM target transfer is missing").into());
    };
    let target_block = lowered
        .entry
        .blocks
        .iter()
        .find(|block| block.id == *target)
        .ok_or_else(|| std::io::Error::other("RETURN-FROM result block is missing"))?;
    assert_eq!(args.len(), target_block.params.len());
    assert_eq!(
        named_call_block(&lowered.entry, "COMMON-LISP", "VALUES-LIST", true),
        Some(target_block.id)
    );
    let returned_values = target_block
        .params
        .first()
        .ok_or_else(|| std::io::Error::other("PROGV return value parameter is missing"))?;
    assert!(target_block.ops.iter().any(|op| {
        matches!(&op.kind, OpKind::CallClosure { args, .. } if args.get(1) == Some(&returned_values.value))
    }));
    assert!(lowered.entry.constants.contains(&Constant::Fixnum(9)));
    assert!(emits_named_call(
        &lowered.entry,
        "COMMON-LISP",
        "VALUES-LIST",
        true,
    ));
    Ok(())
}

#[test]
fn unwind_protect_runs_normal_cleanup_and_preserves_protected_values() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(56);
    let protected_name = fixture.user("PROTECTED");
    let protected = fixture.list(&[protected_name]);
    let cleanup_name = fixture.user("CLEANUP");
    let cleanup = fixture.list(&[cleanup_name]);
    let input = fixture.form("UNWIND-PROTECT", &[protected, cleanup]);
    let expr = fixture.expand(input)?;
    let form = super::common::build(
        &mut builder,
        id,
        &expr,
        "(unwind-protect (protected) (cleanup))",
    )?;
    let lowered = ncl_hir::lower::lower(&form, builder.symbols())?;
    verify_ir(&lowered.entry, "verify normal unwind-protect MIR")?;

    // CLHS requires cleanup forms to run on normal exit while preserving the protected form's values.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_unwind.htm
    assert!(lowered.entry.handler_regions.iter().any(|region| {
        region.kind == ncl_ir::HandlerKind::UnwindProtect
            && region.cleanup.is_some()
            && !region.protected.is_empty()
    }));
    assert!(emits_named_call(
        &lowered.entry,
        "COMMON-LISP-USER",
        "PROTECTED",
        true,
    ));
    assert!(emits_named_call(
        &lowered.entry,
        "COMMON-LISP-USER",
        "CLEANUP",
        true,
    ));
    assert!(emits_named_call(
        &lowered.entry,
        "NCL-EXT",
        "CAPTURE-MULTIPLE-VALUES",
        false,
    ));
    assert!(emits_named_call(
        &lowered.entry,
        "COMMON-LISP",
        "VALUES-LIST",
        true,
    ));
    Ok(())
}

#[test]
fn unwind_protect_runs_cleanup_when_throw_crosses_a_catch() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(57);
    let tag = Word::fixnum(77);
    let throw = fixture.form("THROW", &[tag, Word::fixnum(9)]);
    let cleanup_name = fixture.user("CLEANUP");
    let cleanup = fixture.list(&[cleanup_name]);
    let protected = fixture.form("UNWIND-PROTECT", &[throw, cleanup]);
    let input = fixture.form("CATCH", &[tag, protected]);
    let expr = fixture.expand(input)?;
    let form = super::common::build(
        &mut builder,
        id,
        &expr,
        "(catch 77 (unwind-protect (throw 77 9) (cleanup)))",
    )?;
    let lowered = ncl_hir::lower::lower(&form, builder.symbols())?;
    verify_ir(&lowered.entry, "verify non-local unwind-protect MIR")?;

    // CLHS specifies cleanup execution before a THROW completes its non-local transfer.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_unwind.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/05_b.htm
    assert!(lowered.entry.handler_regions.iter().any(|region| {
        region.kind == ncl_ir::HandlerKind::Catch && !region.protected.is_empty()
    }));
    assert!(lowered.entry.handler_regions.iter().any(|region| {
        region.kind == ncl_ir::HandlerKind::UnwindProtect
            && region.cleanup.is_some()
            && !region.protected.is_empty()
    }));
    assert!(emits_named_call(
        &lowered.entry,
        "COMMON-LISP-USER",
        "CLEANUP",
        true,
    ));
    Ok(())
}

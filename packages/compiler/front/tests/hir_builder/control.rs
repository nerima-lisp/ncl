use super::common::{
    TestResult, context, expand_build, fixture_text, has_node, lower_both_and_verify, verify_ir,
};
use ncl_hir::{MultipleValueOperation, NodeId, NodeKind, Terminator};
use ncl_ir::{Constant, OpKind};
use ncl_object::Word;

#[test]
fn cleanup_and_multiple_value_non_local_exit_are_explicit() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(36);
    let done = fixture.user("DONE");
    let values = fixture.form("VALUES", &[Word::fixnum(1), Word::fixnum(2)]);
    let preserved = fixture.form("MULTIPLE-VALUE-PROG1", &[values, Word::fixnum(3)]);
    let exit = fixture.form("RETURN-FROM", &[done, preserved]);
    let protected = fixture.form("UNWIND-PROTECT", &[exit, Word::fixnum(4)]);
    let input = fixture.form("BLOCK", &[done, protected]);
    let source = fixture_text(
        include_str!("../fixtures/hir-bridge/control.lisp"),
        "(block done\n  (unwind-protect\n      (return-from done (multiple-value-prog1 (values 1 2) 3))\n    4))\n",
    );
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::Block { .. }
    )));
    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::UnwindProtect { .. }
    )));
    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::MultipleValues(MultipleValueOperation::Prog1 { .. })
    )));
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
    Ok(())
}

fn emits_named_call(
    function: &ncl_ir::Function,
    package: &str,
    name: &str,
    named_symbol: bool,
) -> bool {
    let symbol_value = function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|op| {
            let OpKind::Const { result } = &op.kind else {
                return None;
            };
            let constant = function.constants.get(usize::try_from(result.0).ok()?)?;
            if !matches!(
                constant,
                Constant::Symbol {
                    package: symbol_package,
                    name: symbol_name,
                } if symbol_package == package && symbol_name == name
            ) {
                return None;
            }
            op.results.first().map(|(value, _)| *value)
        });
    let Some(symbol_value) = symbol_value else {
        return false;
    };
    let function_value = function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|op| {
            matches!(
                &op.kind,
                OpKind::LoadField { object, field: 1 } if *object == symbol_value
            )
            .then(|| op.results.first().map(|(value, _)| *value))
            .flatten()
        });
    let Some(function_value) = function_value else {
        return false;
    };
    function.blocks.iter().any(|block| {
        block.ops.iter().any(|op| {
            let OpKind::CallClosure {
                closure,
                named_symbol: call_symbol,
                ..
            } = &op.kind
            else {
                return false;
            };
            let symbol_matches = match (named_symbol, call_symbol) {
                (true, Some(call_symbol)) => *call_symbol == symbol_value,
                (false, None) => true,
                _ => false,
            };
            *closure == function_value && symbol_matches
        })
    })
}

#[test]
fn forward_go_and_setq_form_a_verified_tagbody_cfg() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(37);
    let done = fixture.user("DONE");
    let go = fixture.form("GO", &[done]);
    let x = fixture.user("X");
    let setq = fixture.form("SETQ", &[x, Word::fixnum(1)]);
    let input = fixture.form("TAGBODY", &[go, setq, done]);
    let source = fixture_text(
        include_str!("../fixtures/hir-bridge/tagbody.lisp"),
        "(tagbody\n  (go done)\n  (setq x 1)\n  done)\n",
    );
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;
    let (tagbody, target) = form
        .nodes()
        .iter()
        .find_map(|(index, node)| match &node.kind {
            NodeKind::Tagbody { tags, .. } => {
                let tag = tags.first()?;
                let node = NodeId::try_from_index(id, index).ok()?;
                Some((node, tag.block))
            }
            _ => None,
        })
        .ok_or_else(|| std::io::Error::other("tagbody and forward target are missing"))?;
    let flow = form
        .control_flow()
        .function(tagbody)
        .ok_or_else(|| std::io::Error::other("tagbody flow is missing"))?;
    let entry = form
        .control_flow()
        .block(flow.entry())
        .ok_or_else(|| std::io::Error::other("tagbody entry is missing"))?;

    assert!(matches!(entry.terminator(), Terminator::Jump(block) if *block == target));
    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::Assignment { .. }
    )));
    lower_both_and_verify(&builder, &form, &expr)?;
    Ok(())
}

#[test]
fn eval_extension_and_load_time_value_are_retained() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(38);
    let situation = fixture.keyword("EVAL");
    let situations = fixture.list(&[situation]);
    let read_only = fixture.cl("T");
    let load_time_value = fixture.form("LOAD-TIME-VALUE", &[Word::fixnum(3), read_only]);
    let input = fixture.form("EVAL-WHEN", &[situations, load_time_value]);
    let source = fixture_text(
        include_str!("../fixtures/hir-bridge/eval.lisp"),
        "(eval-when (:eval)\n  (load-time-value 3 t))\n",
    );
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(has_node(
        &form,
        |node| matches!(node, NodeKind::CompileTimeForm { situations, .. } if situations.contains(&ncl_hir::CompileTimeSituation::Eval))
    ));
    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::LoadTimeValue {
            read_only: true,
            ..
        }
    )));
    lower_both_and_verify(&builder, &form, &expr)?;
    Ok(())
}

#[test]
fn catch_throw_and_progv_retain_dynamic_control() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(39);
    let tag = fixture.keyword("TAG");
    let throw = fixture.form("THROW", &[tag, Word::fixnum(5)]);
    let catch = fixture.form("CATCH", &[tag, throw]);
    let special = fixture.user("SPECIAL");
    let symbol_list = fixture.list(&[special]);
    let quoted_symbols = fixture.form("QUOTE", &[symbol_list]);
    let value_list = fixture.list(&[Word::fixnum(5)]);
    let quoted_values = fixture.form("QUOTE", &[value_list]);
    let body = fixture.user("SPECIAL");
    let progv = fixture.form("PROGV", &[quoted_symbols, quoted_values, body]);
    let list_name = fixture.cl("LIST");
    let function = fixture.form("FUNCTION", &[list_name]);
    let values = fixture.form("VALUES", &[Word::fixnum(1), Word::fixnum(2)]);
    let multiple_call = fixture.form("MULTIPLE-VALUE-CALL", &[function, values]);
    let input = fixture.form("PROGN", &[catch, progv, multiple_call]);
    let source = fixture_text(
        include_str!("../fixtures/hir-bridge/dynamic-control.lisp"),
        "(progn\n  (catch :tag (throw :tag 5))\n  (progv '(special) '(5) special)\n  (multiple-value-call #'list (values 1 2)))\n",
    );
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::Catch { .. }
    )));
    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::Throw { .. }
    )));
    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::DynamicBinding { .. }
    )));
    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::MultipleValues(MultipleValueOperation::Call { .. })
    )));
    lower_both_and_verify(&builder, &form, &expr)?;
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

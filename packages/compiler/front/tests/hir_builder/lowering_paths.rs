use ncl_hir::{CompileTimeSituation, ExitTarget, NodeKind};
use ncl_ir::{HandlerKind, OpKind, Terminator};
use ncl_object::Word;

use super::common::{
    TestResult, constant_for_value, context, expand_build, has_node, lower_both_and_verify,
    verify_ir,
};
use super::control::{handler_region, named_call_position};

#[test]
fn throw_from_a_local_function_uses_the_active_dynamic_catch() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(74);
    let tag = fixture.keyword("TAG");
    let leave = fixture.user("LEAVE");
    let parameters = fixture.list(&[]);
    let throw = fixture.form("THROW", &[tag, Word::fixnum(41)]);
    let definition = fixture.list(&[leave, parameters, throw]);
    let definitions = fixture.list(&[definition]);
    let call = fixture.form("LEAVE", &[]);
    let flet = fixture.form("FLET", &[definitions, call]);
    let input = fixture.form("CATCH", &[tag, flet]);
    let source = "(catch :tag (flet ((leave () (throw :tag 41))) (leave)))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::Catch { .. }
    )));
    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::Throw { .. }
    )));
    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let catch = handler_region(&lowered.entry, HandlerKind::Catch)?;
    assert!(catch.catch_tag.is_some());
    let call_block = lowered
        .entry
        .blocks
        .iter()
        .find(|block| {
            block
                .ops
                .iter()
                .any(|operation| matches!(&operation.kind, OpKind::CallClosure { .. }))
        })
        .ok_or_else(|| std::io::Error::other("local function call is missing"))?;
    assert!(catch.protected.contains(&call_block.id));

    let thrower = lowered
        .nested
        .iter()
        .find(|function| {
            function
                .blocks
                .iter()
                .any(|block| matches!(&block.terminator, Terminator::Throw { .. }))
        })
        .ok_or_else(|| std::io::Error::other("local function THROW is missing"))?;
    let thrown_value = thrower
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|operation| match &operation.kind {
            OpKind::Builtin { name, args, .. } if name == "throw" => args.get(1).copied(),
            _ => None,
        })
        .ok_or_else(|| std::io::Error::other("THROW value argument is missing"))?;
    assert_eq!(
        constant_for_value(thrower, thrown_value),
        Some(&ncl_ir::Constant::Fixnum(41))
    );
    verify_ir(thrower, "verify local function THROW MIR")?;
    // CLHS CATCH and THROW use dynamic extent, including calls from nested functions.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_catch.htm
    Ok(())
}

#[test]
fn go_out_of_unwind_protect_runs_cleanup_before_tagbody_transfer() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(75);
    let done = fixture.user("DONE");
    let cleanup_name = fixture.user("CLEANUP");
    let cleanup = fixture.list(&[cleanup_name]);
    let go = fixture.form("GO", &[done]);
    let protected = fixture.form("UNWIND-PROTECT", &[go, cleanup]);
    let input = fixture.form("TAGBODY", &[protected, done]);
    let source = "(tagbody (unwind-protect (go done) (cleanup)) done)";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::NonLocalExit {
            target: ExitTarget::Tag(_),
            cleanups,
            ..
        } if cleanups.len() == 1
    )));
    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let region = handler_region(&lowered.entry, HandlerKind::UnwindProtect)?;
    let (block_id, cleanup_index) =
        named_call_position(&lowered.entry, "COMMON-LISP-USER", "CLEANUP", true)
            .ok_or_else(|| std::io::Error::other("cleanup call is missing"))?;
    let block = lowered
        .entry
        .blocks
        .iter()
        .find(|block| block.id == block_id)
        .ok_or_else(|| std::io::Error::other("cleanup block is missing"))?;
    let leave_index = block
        .ops
        .iter()
        .position(|operation| {
            matches!(&operation.kind, OpKind::LeaveHandler { region: id } if *id == region.id)
        })
        .ok_or_else(|| std::io::Error::other("cleanup path does not leave its handler"))?;
    assert!(region.protected.contains(&block.id));
    assert!(cleanup_index < leave_index);
    assert!(matches!(&block.terminator, Terminator::Jump { .. }));
    // CLHS requires UNWIND-PROTECT cleanup forms to run before a GO leaves the protected form.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_unwind.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_tagbod.htm
    Ok(())
}

#[test]
fn throw_in_unwind_cleanup_supersedes_the_protected_form_result() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(79);
    let tag = fixture.keyword("TAG");
    let cleanup_exit = fixture.form("THROW", &[tag, Word::fixnum(41)]);
    let protected = fixture.form("UNWIND-PROTECT", &[Word::fixnum(7), cleanup_exit]);
    let input = fixture.form("CATCH", &[tag, protected]);
    let source = "(catch :tag (unwind-protect 7 (throw :tag 41)))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;
    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let catch = handler_region(&lowered.entry, HandlerKind::Catch)?;
    let (throw_block, thrown_value) = lowered
        .entry
        .blocks
        .iter()
        .find_map(|block| {
            if !matches!(&block.terminator, Terminator::Throw { .. }) {
                return None;
            }
            let value = block
                .ops
                .iter()
                .find_map(|operation| match &operation.kind {
                    OpKind::Builtin { name, args, .. } if name == "throw" => args.get(1).copied(),
                    _ => None,
                })?;
            Some((block, value))
        })
        .ok_or_else(|| std::io::Error::other("cleanup THROW block is missing"))?;
    assert!(catch.protected.contains(&throw_block.id));
    assert_eq!(
        constant_for_value(&lowered.entry, thrown_value),
        Some(&ncl_ir::Constant::Fixnum(41))
    );

    let handler = lowered
        .entry
        .blocks
        .iter()
        .find(|block| block.id == catch.handler)
        .ok_or_else(|| std::io::Error::other("CATCH result handler is missing"))?;
    let caught_value = handler
        .params
        .first()
        .map(|parameter| parameter.value)
        .ok_or_else(|| std::io::Error::other("CATCH value parameter is missing"))?;
    let Terminator::Jump { target, args } = &handler.terminator else {
        return Err(std::io::Error::other("CATCH result transfer is missing").into());
    };
    assert_eq!(args.first(), Some(&caught_value));
    let result = lowered
        .entry
        .blocks
        .iter()
        .find(|block| block.id == *target)
        .ok_or_else(|| std::io::Error::other("CATCH merge block is missing"))?;
    let result_value = result
        .params
        .first()
        .map(|parameter| parameter.value)
        .ok_or_else(|| std::io::Error::other("CATCH merge value is missing"))?;
    assert!(matches!(
        &result.terminator,
        Terminator::Return { values } if values.first() == Some(&result_value)
    ));
    // A non-local exit in cleanup takes effect, and CATCH returns the thrown values.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_unwind.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_catch.htm
    Ok(())
}

#[test]
fn compile_toplevel_only_eval_when_lowers_to_nil_at_execution_time() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(76);
    let situation = fixture.keyword("COMPILE-TOPLEVEL");
    let situations = fixture.list(&[situation]);
    let input = fixture.form("EVAL-WHEN", &[situations, Word::fixnum(9)]);
    let source = "(eval-when (:compile-toplevel) 9)";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(has_node(&form, |node| matches!(
        node,
        NodeKind::CompileTimeForm { situations, .. }
            if situations.contains(&CompileTimeSituation::CompileToplevel)
    )));
    let (lowered, baseline) = lower_both_and_verify(&builder, &form, &expr)?;
    assert!(lowered.entry.constants.contains(&ncl_ir::Constant::Nil));
    assert!(
        !lowered
            .entry
            .constants
            .contains(&ncl_ir::Constant::Fixnum(9))
    );
    assert!(baseline.entry.constants.contains(&ncl_ir::Constant::Nil));
    assert!(
        !baseline
            .entry
            .constants
            .contains(&ncl_ir::Constant::Fixnum(9))
    );
    // CLHS EVAL-WHEN returns NIL when the current situation is not listed.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_eval_w.htm
    Ok(())
}

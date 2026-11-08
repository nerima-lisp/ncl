use ncl_hir::lower::LowerError;
use ncl_hir::{NodeId, NodeKind, VariableTarget};
use ncl_ir::{Compare, Constant, OpKind, Terminator};
use ncl_object::Word;

use super::common::{TestResult, context, expand_build, lower_both_and_verify};

#[test]
fn mutable_lexical_capture_shares_a_value_cell_between_closures_and_parent() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(110);
    let value = fixture.user("VALUE");
    let binding = fixture.list(&[value, Word::fixnum(0)]);
    let bindings = fixture.list(&[binding]);
    let lambda_list = fixture.list(&[]);
    let assignment = fixture.form("SETQ", &[value, Word::fixnum(1)]);
    let lambda = fixture.form("LAMBDA", &[lambda_list, assignment, value]);
    let call = fixture.form("FUNCALL", &[lambda]);
    let input = fixture.form("LET", &[bindings, call, value]);
    let source = "(let ((value 0))\n  (funcall (lambda () (setq value 1) value))\n  value)";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let binding_id = form.nodes().iter().find_map(|(_, node)| match &node.kind {
        NodeKind::Let { bindings, .. } => bindings.first().map(|binding| binding.binding),
        _ => None,
    });
    let binding_id = binding_id.ok_or_else(|| std::io::Error::other("LET binding is missing"))?;
    let binding_info = form
        .bindings()
        .binding(binding_id)
        .ok_or_else(|| std::io::Error::other("LET binding metadata is missing"))?;
    assert!(!binding_info.writes().is_empty());
    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::Assignment {
            target: VariableTarget::Lexical(target),
            ..
        } if *target == binding_id
    )));

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    assert!(lowered.entry.blocks.iter().any(|block| {
        block
            .ops
            .iter()
            .any(|operation| matches!(&operation.kind, OpKind::MakeValueCell { .. }))
    }));
    let closure_captures = lowered
        .entry
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|operation| match &operation.kind {
            OpKind::MakeClosure { captures, .. } => Some(captures.len()),
            _ => None,
        });
    assert_eq!(closure_captures, Some(1));

    let closure = lowered
        .nested
        .first()
        .ok_or_else(|| std::io::Error::other("capturing lambda MIR is missing"))?;
    let captured_cell = closure
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|operation| match &operation.kind {
            OpKind::LoadCapture { index: 0 } => operation.results.first().map(|(value, _)| *value),
            _ => None,
        })
        .ok_or_else(|| std::io::Error::other("captured value cell is missing"))?;
    assert!(closure.blocks.iter().flat_map(|block| &block.ops).any(|operation| {
        matches!(
            &operation.kind,
            OpKind::StoreField {
                object,
                field: 0,
                value,
            } if *object == captured_cell
                && super::common::constant_for_value(closure, *value) == Some(&Constant::Fixnum(1))
        )
    }));
    assert!(
        closure
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .any(|operation| {
                matches!(
                    &operation.kind,
                    OpKind::LoadField { object, field: 0 } if *object == captured_cell
                )
            })
    );

    // CLHS lexical closures retain the binding they close over; SETQ updates that binding.
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_ad.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_setq.htm
    Ok(())
}

#[test]
fn global_and_special_unbound_reads_branch_to_the_unbound_variable_helper() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(111);
    let dynamic = fixture.user("*DYNAMIC-VALUE*");
    let special_head = fixture.cl("SPECIAL");
    let special = fixture.list(&[special_head, dynamic]);
    let declaration = fixture.form("DECLARE", &[special]);
    let global = fixture.user("GLOBAL-VALUE");
    let body = fixture.form("PROGN", &[dynamic, global]);
    let input = fixture.form("LOCALLY", &[declaration, body]);
    let source =
        "(locally (declare (special *dynamic-value*))\n  (progn *dynamic-value* global-value))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let mut global_reads = 0;
    let mut special_reads = 0;
    for (_, node) in form.nodes().iter() {
        match &node.kind {
            NodeKind::VariableReference {
                target: VariableTarget::Global(_),
            } => global_reads += 1,
            NodeKind::VariableReference {
                target: VariableTarget::Special(_),
            } => special_reads += 1,
            _ => {}
        }
    }
    assert_eq!(global_reads, 1);
    assert_eq!(special_reads, 1);

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let operations = lowered.entry.blocks.iter().flat_map(|block| &block.ops);
    let unbound_checks = operations
        .clone()
        .filter(|operation| {
            matches!(
                &operation.kind,
                OpKind::Compare {
                    op: Compare::Eq,
                    left,
                    right,
                } if super::common::constant_for_value(&lowered.entry, *left) == Some(&Constant::Unbound)
                    || super::common::constant_for_value(&lowered.entry, *right) == Some(&Constant::Unbound)
            )
        })
        .count();
    let unbound_helper_calls = operations
        .filter(|operation| {
            matches!(
                &operation.kind,
                OpKind::CallClosure {
                    named_symbol: Some(helper),
                    ..
                } if matches!(
                    super::common::constant_for_value(&lowered.entry, *helper),
                    Some(Constant::Symbol { package, name })
                        if package == "NCL-EXT" && name == "BOUND-SYMBOL-VALUE"
                )
            )
        })
        .count();
    let unbound_branches = lowered
        .entry
        .blocks
        .iter()
        .filter(|block| matches!(&block.terminator, Terminator::Branch { .. }))
        .count();

    assert_eq!(unbound_checks, 2);
    assert_eq!(unbound_helper_calls, 2);
    assert_eq!(unbound_branches, 2);

    // CLHS specifies that reading an unbound variable signals UNBOUND-VARIABLE.
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_abaa.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/e_unbo_1.htm
    Ok(())
}

#[test]
fn nonliteral_load_time_value_fails_closed_during_hir_lowering() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(112);
    let addition = fixture.form("+", &[Word::fixnum(1), Word::fixnum(2)]);
    let input = fixture.form("LOAD-TIME-VALUE", &[addition]);
    let source = "(load-time-value (+ 1 2))";
    let (_, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let (load_time_value, form_node) = form
        .nodes()
        .iter()
        .find(|(_, node)| matches!(&node.kind, NodeKind::LoadTimeValue { .. }))
        .ok_or_else(|| std::io::Error::other("LOAD-TIME-VALUE HIR node is missing"))?;
    let load_time_value = NodeId::try_from_index(id, load_time_value)?;
    let NodeKind::LoadTimeValue {
        form: runtime_form,
        read_only: false,
    } = &form_node.kind
    else {
        return Err(
            std::io::Error::other("LOAD-TIME-VALUE did not retain its default mode").into(),
        );
    };
    assert!(matches!(
        form.nodes().get(*runtime_form).map(|node| &node.kind),
        Some(NodeKind::Call { .. })
    ));

    let Err(error) = ncl_hir::lower::lower(&form, builder.symbols()) else {
        return Err(std::io::Error::other(
            "runtime LOAD-TIME-VALUE must not be lowered as an ordinary call",
        )
        .into());
    };
    assert_eq!(
        error,
        LowerError::Unrepresentable {
            node: load_time_value,
            operation: "load-time-value-with-runtime-form",
        }
    );

    // CLHS LOAD-TIME-VALUE evaluates its form at compile time or load time, which this MIR bridge cannot model.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_ld_tim.htm
    Ok(())
}

use ncl_ir::{Constant, OpKind, Terminator as MirTerminator};
use ncl_object::Word;

use super::common::{
    TestResult, build, constant_for_value, context, has_node, lower_both_and_verify,
};
use super::control::named_call_position;

#[test]
fn multiple_value_call_passes_each_returned_value_to_its_function() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(62);
    let list_name = fixture.cl("LIST");
    let function = fixture.form("FUNCTION", &[list_name]);
    let values = fixture.form("VALUES", &[Word::fixnum(1), Word::fixnum(2)]);
    let input = fixture.form("MULTIPLE-VALUE-CALL", &[function, values]);
    let expr = fixture.expand(input)?;
    let form = build(
        &mut builder,
        id,
        &expr,
        "(multiple-value-call #'list (values 1 2))",
    )?;
    assert!(has_node(&form, |node| matches!(
        node,
        ncl_hir::NodeKind::MultipleValues(ncl_hir::MultipleValueOperation::Call { .. })
    )));
    assert!(has_node(&form, |node| matches!(
        node,
        ncl_hir::NodeKind::MultipleValues(ncl_hir::MultipleValueOperation::Produce { values })
            if values.len() == 2
    )));
    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;

    // CLHS MULTIPLE-VALUE-CALL passes every value from each form to its function.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_multip.htm
    let produced_values = lowered
        .entry
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|operation| match &operation.kind {
            OpKind::SetMultipleValues { values } if values.len() == 2 => Some(values),
            _ => None,
        })
        .ok_or_else(|| std::io::Error::other("two produced values are missing"))?;
    let first_value = produced_values
        .first()
        .copied()
        .ok_or_else(|| std::io::Error::other("primary value is missing"))?;
    let second_value = produced_values
        .get(1)
        .copied()
        .ok_or_else(|| std::io::Error::other("secondary value is missing"))?;
    assert_eq!(
        constant_for_value(&lowered.entry, first_value),
        Some(&Constant::Fixnum(1))
    );
    assert_eq!(
        constant_for_value(&lowered.entry, second_value),
        Some(&Constant::Fixnum(2))
    );
    let (capture_block, capture_index) =
        named_call_position(&lowered.entry, "NCL-EXT", "CAPTURE-MULTIPLE-VALUES", false)
            .ok_or_else(|| std::io::Error::other("multiple-value capture is missing"))?;
    let (call_block, call_index) =
        named_call_position(&lowered.entry, "NCL-EXT", "MULTIPLE-VALUE-CALL-LIST", false)
            .ok_or_else(|| std::io::Error::other("multiple-value function call is missing"))?;
    assert_eq!(capture_block, call_block);
    let block = lowered
        .entry
        .blocks
        .iter()
        .find(|block| block.id == call_block)
        .ok_or_else(|| std::io::Error::other("multiple-value call block is missing"))?;
    let capture_operation = block
        .ops
        .get(capture_index)
        .ok_or_else(|| std::io::Error::other("capture operation is missing"))?;
    let OpKind::CallClosure {
        args: capture_args, ..
    } = &capture_operation.kind
    else {
        return Err(std::io::Error::other("capture is not a closure call").into());
    };
    assert_eq!(capture_args.get(1), Some(&first_value));
    let captured_values = capture_operation
        .results
        .first()
        .map(|(value, _)| *value)
        .ok_or_else(|| std::io::Error::other("captured value list is missing"))?;
    let OpKind::CallClosure {
        args: call_args, ..
    } = &block
        .ops
        .get(call_index)
        .ok_or_else(|| std::io::Error::other("multiple-value call is missing"))?
        .kind
    else {
        return Err(std::io::Error::other("multiple-value call is not a closure call").into());
    };
    assert_eq!(call_args.get(2), Some(&captured_values));
    Ok(())
}

#[test]
fn zero_values_use_nil_as_the_primary_value() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(63);
    let input = fixture.form("VALUES", &[]);
    let expr = fixture.expand(input)?;
    let form = build(&mut builder, id, &expr, "(values)")?;
    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;

    // CLHS specifies that an unrequested zero-value result supplies NIL to the caller.
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_ag.htm
    assert!(lowered.entry.constants.contains(&Constant::Nil));
    assert!(lowered.entry.blocks.iter().any(|block| {
        block.ops.iter().any(|operation| {
            matches!(&operation.kind, OpKind::SetMultipleValues { values } if values.is_empty())
        })
    }));
    let returned = lowered
        .entry
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            MirTerminator::Return { values } => values.first().copied(),
            _ => None,
        })
        .ok_or_else(|| std::io::Error::other("single-value result is missing"))?;
    assert_eq!(
        constant_for_value(&lowered.entry, returned),
        Some(&Constant::Nil)
    );
    Ok(())
}

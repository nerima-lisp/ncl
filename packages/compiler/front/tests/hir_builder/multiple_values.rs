use ncl_ir::{Constant, OpKind};
use ncl_object::Word;

use super::common::{TestResult, build, context, has_node, lower_both_and_verify};

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
    assert!(lowered.entry.constants.contains(&Constant::Fixnum(1)));
    assert!(lowered.entry.constants.contains(&Constant::Fixnum(2)));
    for name in ["CAPTURE-MULTIPLE-VALUES", "MULTIPLE-VALUE-CALL-LIST"] {
        assert!(lowered.entry.constants.iter().any(|constant| matches!(
            constant,
            Constant::Symbol { package, name: actual }
                if package == "NCL-EXT" && actual == name
        )));
    }
    assert!(lowered.entry.blocks.iter().any(|block| {
        block.ops.iter().any(|operation| {
            matches!(&operation.kind, OpKind::CallClosure { args, named_symbol: None, .. } if args.len() == 3)
        })
    }));
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
    Ok(())
}

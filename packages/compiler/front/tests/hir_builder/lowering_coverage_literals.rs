use ncl_hir::{ClTypeKind, Literal as HirLiteral, NodeKind, NumberLiteral};
use ncl_ir::{Constant, OpKind, Terminator as MirTerminator};
use ncl_object::{Word, make_bignum_from_i128, make_complex, make_ratio};

use super::common::{TestResult, context, expand_build, lower_both_and_verify};

#[test]
fn read_only_load_time_value_accepts_a_string_literal() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(120);
    let string = fixture.string("immutable");
    let read_only = fixture.cl("T");
    let input = fixture.form("LOAD-TIME-VALUE", &[string, read_only]);
    let source = "(load-time-value \"immutable\" t)";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::LoadTimeValue {
            read_only: true,
            form: value_form,
        } if matches!(form.nodes().get(*value_form).map(|node| &node.kind), Some(NodeKind::Literal(_)))
    )));

    let (lowered, baseline) = lower_both_and_verify(&builder, &form, &expr)?;
    let string_constant = Constant::StringBytes(b"immutable".to_vec());
    assert!(lowered.entry.constants.contains(&string_constant));
    assert!(baseline.entry.constants.contains(&string_constant));

    // CLHS permits a read-only LOAD-TIME-VALUE result to be treated as a constant object.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_ld_tim.htm
    Ok(())
}

#[test]
fn bignum_integer_bounds_remain_opaque_and_still_emit_checked_type_mir() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(131);
    let large_value = (1_i128) << 70;
    let large_bound: Word =
        make_bignum_from_i128(&mut fixture.ctx, &fixture.runtime, large_value)?.into();
    let star = fixture.cl("*");
    let lower_bounded = fixture.form("INTEGER", &[large_bound, star]);
    let lower_assertion = fixture.form("THE", &[lower_bounded, Word::fixnum(7)]);
    let upper_bounded = fixture.form("INTEGER", &[star, large_bound]);
    let upper_assertion = fixture.form("THE", &[upper_bounded, Word::fixnum(11)]);
    let input = fixture.form("PROGN", &[lower_assertion, upper_assertion]);
    let source = "(progn (the (integer 1180591620717411303424 *) 7) (the (integer * 1180591620717411303424) 11))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let asserted_types = form
        .nodes()
        .iter()
        .filter_map(|(_, node)| match &node.kind {
            NodeKind::TypeAssertion { asserted_type, .. } => Some(*asserted_type),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(asserted_types.len(), 2);
    assert!(asserted_types.iter().all(|ty| matches!(
        form.types().get(*ty).map(ncl_hir::ClType::kind),
        Some(ClTypeKind::Opaque { .. })
    )));

    let (lowered, _) = lower_both_and_verify(&builder, &form, &expr)?;
    let mut operations = lowered.entry.blocks.iter().flat_map(|block| &block.ops);
    assert!(operations.clone().any(|operation| matches!(
        &operation.kind,
        OpKind::Prim {
            op: ncl_ir::Prim::Typep,
            ..
        }
    )));
    assert!(operations.any(|operation| matches!(
        &operation.kind,
        OpKind::Builtin { name, .. } if name == "ncl_signal_type_error"
    )));
    assert_eq!(
        lowered
            .entry
            .blocks
            .iter()
            .filter(|block| matches!(&block.terminator, MirTerminator::Branch { .. }))
            .count(),
        2
    );

    // CLHS INTEGER type specifiers accept arbitrary integer bounds, including bignums.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    Ok(())
}

#[test]
fn complex_literals_with_ratio_components_survive_both_lowerers() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(132);
    let half: Word = make_ratio(
        &mut fixture.ctx,
        &fixture.runtime,
        Word::fixnum(1),
        Word::fixnum(2),
    )?
    .into();
    let three_quarters: Word = make_ratio(
        &mut fixture.ctx,
        &fixture.runtime,
        Word::fixnum(3),
        Word::fixnum(4),
    )?
    .into();
    let complex: Word =
        make_complex(&mut fixture.ctx, &fixture.runtime, half, three_quarters)?.into();
    let input = fixture.form("PROGN", &[complex]);
    let source = "(progn #C(1/2 3/4))";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    assert!(form.nodes().iter().any(|(_, node)| matches!(
        &node.kind,
        NodeKind::Literal(HirLiteral::Number(NumberLiteral::Complex { real, imaginary }))
            if matches!(real.as_ref(), NumberLiteral::Ratio { .. })
                && matches!(imaginary.as_ref(), NumberLiteral::Ratio { .. })
    )));

    let (lowered, baseline) = lower_both_and_verify(&builder, &form, &expr)?;
    assert!(
        lowered
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Complex { .. }))
    );
    assert!(
        baseline
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Complex { .. }))
    );

    // CLHS #C syntax constructs complex numbers from real numeric components.
    // https://www.lispworks.com/documentation/HyperSpec/Body/02_ca.htm
    Ok(())
}

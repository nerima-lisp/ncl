use super::*;

#[test]
fn normal_catch_exit_returns_the_body_value_and_leaves_its_handler() -> TestResult<()> {
    let (_, mut builder, symbols) = context(1, "(catch 7 33)")?;
    let scope = builder.root_scope();
    let tag = literal(
        &mut builder,
        scope,
        Literal::Number(ncl_hir::NumberLiteral::Fixnum(7)),
    )?;
    let body = literal(
        &mut builder,
        scope,
        Literal::Number(ncl_hir::NumberLiteral::Fixnum(33)),
    )?;
    let catch = builder.alloc_node(
        scope,
        NodeKind::Catch {
            tag,
            body: vec![body],
        },
    )?;
    builder.add_root(catch)?;

    let form = builder.finish()?;
    let lowered = ncl_hir::lower::lower(&form, &symbols)?;
    let returned = returned_value(&lowered.entry)
        .ok_or_else(|| std::io::Error::other("CATCH body value is not returned"))?;
    let merge = lowered
        .entry
        .blocks
        .iter()
        .find(|block| {
            block
                .params
                .iter()
                .any(|parameter| parameter.value == returned)
        })
        .ok_or_else(|| std::io::Error::other("CATCH merge block is missing"))?;
    assert!(matches!(
        &merge.terminator,
        Terminator::Return { values } if values.as_slice() == [returned]
    ));
    let region = lowered
        .entry
        .handler_regions
        .iter()
        .find(|region| region.kind == HandlerKind::Catch)
        .ok_or_else(|| std::io::Error::other("CATCH handler region is missing"))?;
    let normal_exit = lowered
        .entry
        .blocks
        .iter()
        .find(|block| {
            matches!(&block.terminator,
                Terminator::Jump { target, args } if *target == merge.id
                    && args.first().and_then(|value| constant_for_value(&lowered.entry, *value))
                        == Some(&Constant::Fixnum(33)))
        })
        .ok_or_else(|| std::io::Error::other("CATCH body value does not reach its merge"))?;
    assert!(region.protected.contains(&normal_exit.id));
    assert!(normal_exit.ops.iter().any(
        |operation| matches!(&operation.kind, OpKind::LeaveHandler { region: id } if *id == region.id)
    ));
    let tag = region
        .catch_tag
        .ok_or_else(|| std::io::Error::other("CATCH tag is missing"))?;
    assert_eq!(
        constant_for_value(&lowered.entry, tag),
        Some(&Constant::Fixnum(7))
    );
    // CLHS CATCH returns the body values when no matching THROW occurs.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_catch.htm
    Ok(())
}

#[test]
fn return_from_leaves_progv_before_restoring_the_returned_value() -> TestResult<()> {
    let lowered = lower_progv_return()?;
    let region = lowered
        .entry
        .handler_regions
        .iter()
        .find(|region| region.kind == HandlerKind::Progv)
        .ok_or_else(|| std::io::Error::other("PROGV handler region is missing"))?;
    let (exit_block, leave_position) = lowered
        .entry
        .blocks
        .iter()
        .find_map(|block| {
            block.ops.iter().position(|operation| {
                matches!(&operation.kind, OpKind::LeaveHandler { region: id } if *id == region.id)
            })
            .map(|position| (block, position))
        })
        .ok_or_else(|| std::io::Error::other("RETURN-FROM does not leave PROGV"))?;
    assert!(region.protected.contains(&exit_block.id));
    let capture_args =
        named_call_arguments(&lowered.entry, "NCL-EXT", "CAPTURE-MULTIPLE-VALUES", false)
            .ok_or_else(|| std::io::Error::other("RETURN-FROM value capture is missing"))?;
    let captured_values =
        named_call_result_with_mode(&lowered.entry, "NCL-EXT", "CAPTURE-MULTIPLE-VALUES", false)
            .ok_or_else(|| std::io::Error::other("captured multiple values are missing"))?;
    let captured_value = *capture_args
        .get(1)
        .ok_or_else(|| std::io::Error::other("RETURN-FROM value is not captured"))?;
    assert_eq!(
        constant_for_value(&lowered.entry, captured_value),
        Some(&Constant::Fixnum(73))
    );
    let (exit_target, exit_args) = match &exit_block.terminator {
        Terminator::Jump { target, args } => (*target, args),
        _ => return Err(std::io::Error::other("RETURN-FROM transfer is missing").into()),
    };
    let capture_position = exit_block
        .ops
        .iter()
        .position(|operation| {
            operation
                .results
                .iter()
                .any(|(value, _)| *value == captured_values)
        })
        .ok_or_else(|| {
            std::io::Error::other("captured values are not available in the exit block")
        })?;
    assert!(capture_position < leave_position);
    assert_eq!(exit_args.as_slice(), &[captured_values]);
    let merge = lowered
        .entry
        .blocks
        .iter()
        .find(|block| block.id == exit_target)
        .ok_or_else(|| std::io::Error::other("BLOCK exit merge is missing"))?;
    let values_list_result = named_call_result(&lowered.entry, "COMMON-LISP", "VALUES-LIST")
        .ok_or_else(|| std::io::Error::other("returned multiple values are not restored"))?;
    assert!(merge.ops.iter().any(|operation| {
        operation
            .results
            .iter()
            .any(|(value, _)| *value == values_list_result)
    }));
    assert_eq!(returned_value(&lowered.entry), Some(values_list_result));
    // CLHS requires PROGV's dynamic bindings to end when RETURN-FROM leaves its body.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_progv.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_block.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/05_b.htm
    Ok(())
}

fn lower_progv_return() -> TestResult<ncl_hir::lower::Lowered> {
    let (form_id, mut builder, mut symbols) = context(
        4,
        "(block done (progv '(special) '(5) (return-from done 73)))",
    )?;
    let scope = builder.root_scope();
    let block_name = symbols.intern(form_id, Some("COMMON-LISP-USER"), "DONE", None)?;
    let special = symbols.intern(form_id, Some("COMMON-LISP-USER"), "SPECIAL", None)?;
    builder.register_symbol(block_name)?;
    builder.register_symbol(special)?;
    let symbols_list = literal(
        &mut builder,
        scope,
        Literal::Cons(Box::new(Literal::Symbol(special)), Box::new(Literal::Nil)),
    )?;
    let values_list = literal(
        &mut builder,
        scope,
        Literal::Cons(
            Box::new(Literal::Number(ncl_hir::NumberLiteral::Fixnum(5))),
            Box::new(Literal::Nil),
        ),
    )?;
    let returned_form = literal(
        &mut builder,
        scope,
        Literal::Number(ncl_hir::NumberLiteral::Fixnum(73)),
    )?;
    let target_index = builder
        .nodes_mut()
        .len()
        .checked_add(2)
        .ok_or_else(|| std::io::Error::other("HIR node index overflow"))?;
    let target = NodeId::try_from_index(form_id, target_index)?;
    let return_from = builder.alloc_node(
        scope,
        NodeKind::ReturnFrom {
            target,
            value: Some(returned_form),
        },
    )?;
    let progv = builder.alloc_node(
        scope,
        NodeKind::DynamicBinding {
            symbols: symbols_list,
            values: values_list,
            body: vec![return_from],
        },
    )?;
    let block = builder.alloc_node(
        scope,
        NodeKind::Block {
            name: block_name,
            body: vec![progv],
        },
    )?;
    assert_eq!(block, target);
    builder.add_root(block)?;

    let form = builder.finish()?;
    Ok(ncl_hir::lower::lower(&form, &symbols)?)
}

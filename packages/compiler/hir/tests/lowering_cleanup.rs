//! Behavioral tests for cleanup paths in HIR-to-MIR lowering.

use std::error::Error;

use ncl_hir::{
    CallTarget, CleanupId, CompilationUnitId, ExitTarget, FormId, HirFormBuilder, Literal, NodeId,
    NodeKind, NumberLiteral, SourceFiles, SourceOrigins, SymbolId, SymbolTable,
};
use ncl_ir::{Constant, HandlerKind, OpKind, Terminator as MirTerminator, ValueId};

type TestResult<T> = Result<T, Box<dyn Error>>;

fn context(form_index: u32, source: &str) -> TestResult<(FormId, HirFormBuilder, SymbolTable)> {
    let compilation_unit = CompilationUnitId::new(0);
    let form = FormId::new(compilation_unit, form_index);
    let mut files = SourceFiles::new(compilation_unit);
    let file = files.add("lowering-cleanup.lisp", source)?;
    let span = files.span(file, 0, source.len())?;
    let mut origins = SourceOrigins::new(form);
    let origin = origins.add_source(&files, file, span)?;
    let builder = HirFormBuilder::new(form, origins, origin)?;
    Ok((form, builder, SymbolTable::new(compilation_unit)))
}

fn intern_symbol(
    form: FormId,
    builder: &mut HirFormBuilder,
    symbols: &mut SymbolTable,
    package: &str,
    name: &str,
) -> TestResult<SymbolId> {
    let symbol = symbols.intern(form, Some(package), name, None)?;
    builder.register_symbol(symbol)?;
    Ok(symbol)
}

fn literal(
    builder: &mut HirFormBuilder,
    scope: ncl_hir::ScopeId,
    value: Literal,
) -> TestResult<NodeId> {
    Ok(builder.alloc_node(scope, NodeKind::Literal(value))?)
}

fn constant_for_value(function: &ncl_ir::Function, value: ValueId) -> Option<&Constant> {
    let operation = function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find(|operation| operation.results.iter().any(|(result, _)| *result == value))?;
    match &operation.kind {
        OpKind::Const { result } => function.constants.get(usize::try_from(result.0).ok()?),
        OpKind::Convert { value, .. } | OpKind::Move { value } => {
            constant_for_value(function, *value)
        }
        _ => None,
    }
}

fn constant_at(function: &ncl_ir::Function, index: ncl_ir::ConstantIndex) -> Option<&Constant> {
    function.constants.get(usize::try_from(index.0).ok()?)
}

fn named_call_position(
    function: &ncl_ir::Function,
    package: &str,
    name: &str,
    named_symbol: bool,
) -> Option<(ncl_ir::BlockId, usize)> {
    let symbol_value = function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|operation| {
            let OpKind::Const { result } = &operation.kind else {
                return None;
            };
            let constant = constant_at(function, *result)?;
            if !matches!(constant, Constant::Symbol { package: actual_package, name: actual_name }
                if actual_package == package && actual_name == name)
            {
                return None;
            }
            operation.results.first().map(|(value, _)| *value)
        })?;
    let function_value = function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|operation| {
            matches!(
                &operation.kind,
                OpKind::LoadField { object, field: 1 } if *object == symbol_value
            )
            .then(|| operation.results.first().map(|(value, _)| *value))
            .flatten()
        })?;
    function.blocks.iter().find_map(|block| {
        block
            .ops
            .iter()
            .position(|operation| {
                matches!(
                    &operation.kind,
                    OpKind::CallClosure {
                        closure,
                        named_symbol: call_symbol,
                        ..
                    } if *closure == function_value
                        && match (named_symbol, call_symbol) {
                            (true, Some(call_symbol)) => *call_symbol == symbol_value,
                            (false, None) => true,
                            _ => false,
                        }
                )
            })
            .map(|position| (block.id, position))
    })
}

#[test]
fn unwind_protect_runs_normal_cleanup_before_returning_protected_values() -> TestResult<()> {
    let source = "(unwind-protect 19 (cleanup))";
    let (form_id, mut builder, mut symbols) = context(2, source)?;
    let scope = builder.root_scope();
    let cleanup_name = intern_symbol(
        form_id,
        &mut builder,
        &mut symbols,
        "COMMON-LISP-USER",
        "CLEANUP",
    )?;
    let protected = literal(
        &mut builder,
        scope,
        Literal::Number(NumberLiteral::Fixnum(19)),
    )?;
    let cleanup_call = builder.alloc_node(
        scope,
        NodeKind::Call {
            target: CallTarget::Global(cleanup_name),
            arguments: Vec::new(),
        },
    )?;
    let cleanup = CleanupId::try_from_index(form_id, 0)?;
    let unwind_protect = builder.alloc_node(
        scope,
        NodeKind::UnwindProtect {
            cleanup,
            protected,
            cleanup_forms: vec![cleanup_call],
        },
    )?;
    builder.add_root(unwind_protect)?;

    let form = builder.finish()?;
    let lowered = ncl_hir::lower::lower(&form, &symbols)?;

    // CLHS specifies that cleanup forms run on normal exit while UNWIND-PROTECT
    // returns the protected form's values.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_unwind.htm
    let region = lowered
        .entry
        .handler_regions
        .iter()
        .find(|region| region.kind == HandlerKind::UnwindProtect)
        .ok_or_else(|| std::io::Error::other("UNWIND-PROTECT region is missing"))?;
    let (cleanup_block_id, cleanup_position) =
        named_call_position(&lowered.entry, "COMMON-LISP-USER", "CLEANUP", true)
            .ok_or_else(|| std::io::Error::other("normal cleanup call is missing"))?;
    assert!(region.protected.contains(&cleanup_block_id));
    let body = lowered
        .entry
        .blocks
        .iter()
        .find(|block| block.id == cleanup_block_id)
        .ok_or_else(|| std::io::Error::other("normal cleanup block is missing"))?;
    let capture_position =
        named_call_position(&lowered.entry, "NCL-EXT", "CAPTURE-MULTIPLE-VALUES", false)
            .filter(|(block_id, _)| *block_id == body.id)
            .map(|(_, position)| position)
            .ok_or_else(|| std::io::Error::other("protected values are not captured"))?;
    let restore_position = named_call_position(&lowered.entry, "COMMON-LISP", "VALUES-LIST", true)
        .filter(|(block_id, _)| *block_id == body.id)
        .map(|(_, position)| position)
        .ok_or_else(|| std::io::Error::other("protected values are not restored"))?;
    let capture = &body.ops[capture_position];
    let OpKind::CallClosure {
        args: capture_args, ..
    } = &capture.kind
    else {
        return Err(std::io::Error::other("multiple-value capture call is invalid").into());
    };
    let protected_value = *capture_args
        .get(1)
        .ok_or_else(|| std::io::Error::other("protected value is not captured"))?;
    assert_eq!(
        constant_for_value(&lowered.entry, protected_value),
        Some(&Constant::Fixnum(19))
    );
    let captured_values = capture
        .results
        .first()
        .map(|(value, _)| *value)
        .ok_or_else(|| std::io::Error::other("captured multiple-value list is missing"))?;
    let cleanup_result = body.ops[cleanup_position]
        .results
        .first()
        .map(|(value, _)| *value)
        .ok_or_else(|| std::io::Error::other("cleanup call result is missing"))?;
    let restore = &body.ops[restore_position];
    let OpKind::CallClosure {
        args: restore_args, ..
    } = &restore.kind
    else {
        return Err(std::io::Error::other("VALUES-LIST restoration call is invalid").into());
    };
    assert_eq!(restore_args.get(1), Some(&captured_values));
    let restored_value = restore
        .results
        .first()
        .map(|(value, _)| *value)
        .ok_or_else(|| std::io::Error::other("restored protected value is missing"))?;
    let leave_position = body
        .ops
        .iter()
        .position(|operation| {
            matches!(&operation.kind, OpKind::LeaveHandler { region: id } if *id == region.id)
        })
        .ok_or_else(|| std::io::Error::other("normal path does not leave UNWIND-PROTECT"))?;
    assert!(leave_position < capture_position);
    assert!(capture_position < cleanup_position && cleanup_position < restore_position);
    assert_ne!(cleanup_result, restored_value);
    let MirTerminator::Jump { target, args } = &body.terminator else {
        return Err(std::io::Error::other("normal UNWIND-PROTECT merge is missing").into());
    };
    assert_eq!(args.first(), Some(&restored_value));
    let merge = lowered
        .entry
        .blocks
        .iter()
        .find(|block| block.id == *target)
        .ok_or_else(|| std::io::Error::other("UNWIND-PROTECT merge block is missing"))?;
    assert!(matches!(
        &merge.terminator,
        MirTerminator::Return { values } if values.first() == Some(&protected_value)
    ));
    Ok(())
}

#[test]
fn non_local_block_exit_runs_cleanup_before_transferring_values() -> TestResult<()> {
    let source = "(block done (unwind-protect (return-from done 23) (cleanup)))";
    let (form_id, mut builder, mut symbols) = context(3, source)?;
    let scope = builder.root_scope();
    let block_name = intern_symbol(
        form_id,
        &mut builder,
        &mut symbols,
        "COMMON-LISP-USER",
        "DONE",
    )?;
    let cleanup_name = intern_symbol(
        form_id,
        &mut builder,
        &mut symbols,
        "COMMON-LISP-USER",
        "CLEANUP",
    )?;
    let protected_value = literal(
        &mut builder,
        scope,
        Literal::Number(NumberLiteral::Fixnum(23)),
    )?;
    let cleanup_call = builder.alloc_node(
        scope,
        NodeKind::Call {
            target: CallTarget::Global(cleanup_name),
            arguments: Vec::new(),
        },
    )?;
    let cleanup = CleanupId::try_from_index(form_id, 0)?;
    let block_index = builder
        .nodes_mut()
        .len()
        .checked_add(2)
        .ok_or_else(|| std::io::Error::other("HIR node index overflow"))?;
    let block_target = NodeId::try_from_index(form_id, block_index)?;
    let exit = builder.alloc_node(
        scope,
        NodeKind::NonLocalExit {
            target: ExitTarget::Block(block_target),
            values: vec![protected_value],
            cleanups: vec![cleanup],
        },
    )?;
    let unwind_protect = builder.alloc_node(
        scope,
        NodeKind::UnwindProtect {
            cleanup,
            protected: exit,
            cleanup_forms: vec![cleanup_call],
        },
    )?;
    let block = builder.alloc_node(
        scope,
        NodeKind::Block {
            name: block_name,
            body: vec![unwind_protect],
        },
    )?;
    assert_eq!(block, block_target);
    builder.add_root(block)?;

    let form = builder.finish()?;
    let lowered = ncl_hir::lower::lower(&form, &symbols)?;

    // CLHS requires UNWIND-PROTECT cleanup forms to run before a non-local transfer.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_unwind.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_block.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/05_b.htm
    let region = lowered
        .entry
        .handler_regions
        .iter()
        .find(|region| region.kind == HandlerKind::UnwindProtect)
        .ok_or_else(|| std::io::Error::other("UNWIND-PROTECT region is missing"))?;
    let (cleanup_block_id, cleanup_position) =
        named_call_position(&lowered.entry, "COMMON-LISP-USER", "CLEANUP", true)
            .ok_or_else(|| std::io::Error::other("non-local cleanup call is missing"))?;
    assert!(region.protected.contains(&cleanup_block_id));
    assert_ne!(Some(cleanup_block_id), region.cleanup);
    let cleanup_block = lowered
        .entry
        .blocks
        .iter()
        .find(|block| block.id == cleanup_block_id)
        .ok_or_else(|| std::io::Error::other("non-local cleanup block is missing"))?;
    let capture_position =
        named_call_position(&lowered.entry, "NCL-EXT", "CAPTURE-MULTIPLE-VALUES", false)
            .filter(|(block_id, _)| *block_id == cleanup_block.id)
            .map(|(_, position)| position)
            .ok_or_else(|| std::io::Error::other("non-local exit values are not captured"))?;
    let capture = &cleanup_block.ops[capture_position];
    let OpKind::CallClosure {
        args: capture_args, ..
    } = &capture.kind
    else {
        return Err(std::io::Error::other("non-local value capture call is invalid").into());
    };
    let exit_value = *capture_args
        .get(1)
        .ok_or_else(|| std::io::Error::other("non-local exit value is missing"))?;
    assert_eq!(
        constant_for_value(&lowered.entry, exit_value),
        Some(&Constant::Fixnum(23))
    );
    let captured_values = capture
        .results
        .first()
        .map(|(value, _)| *value)
        .ok_or_else(|| std::io::Error::other("captured exit values are missing"))?;
    assert!(capture_position < cleanup_position);
    let cleanup_result = cleanup_block.ops[cleanup_position]
        .results
        .first()
        .map(|(value, _)| *value)
        .ok_or_else(|| std::io::Error::other("cleanup call result is missing"))?;
    assert!(cleanup_block.ops.iter().enumerate().any(|(position, operation)| {
        position > cleanup_position
            && matches!(&operation.kind, OpKind::LeaveHandler { region: id } if *id == region.id)
    }));
    let MirTerminator::Jump { target, args } = &cleanup_block.terminator else {
        return Err(std::io::Error::other("non-local transfer after cleanup is missing").into());
    };
    assert_eq!(args.first(), Some(&captured_values));
    assert_ne!(args.first(), Some(&cleanup_result));
    let destination = lowered
        .entry
        .blocks
        .iter()
        .find(|block| block.id == *target)
        .ok_or_else(|| std::io::Error::other("non-local transfer target is missing"))?;
    let values_parameter = destination
        .params
        .first()
        .map(|parameter| parameter.value)
        .ok_or_else(|| std::io::Error::other("non-local transfer value parameter is missing"))?;
    let restore_position = named_call_position(&lowered.entry, "COMMON-LISP", "VALUES-LIST", true)
        .filter(|(block_id, _)| *block_id == destination.id)
        .map(|(_, position)| position)
        .ok_or_else(|| std::io::Error::other("transferred values are not restored"))?;
    let restore = &destination.ops[restore_position];
    let OpKind::CallClosure {
        args: restore_args, ..
    } = &restore.kind
    else {
        return Err(std::io::Error::other("transferred value restoration call is invalid").into());
    };
    assert_eq!(restore_args.get(1), Some(&values_parameter));
    let restored_value = restore
        .results
        .first()
        .map(|(value, _)| *value)
        .ok_or_else(|| std::io::Error::other("transferred value result is missing"))?;
    assert!(matches!(
        &destination.terminator,
        MirTerminator::Return { values } if values.first() == Some(&restored_value)
    ));
    Ok(())
}

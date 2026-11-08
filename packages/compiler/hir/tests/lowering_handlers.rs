//! Behavioral tests for handler-related HIR-to-MIR lowering.

use std::error::Error;

use ncl_hir::{
    CompilationUnitId, FormId, HirFormBuilder, Literal, NodeId, NodeKind, NumberLiteral,
    SourceFiles, SourceOrigins, SymbolId, SymbolTable,
};
use ncl_ir::{Constant, HandlerKind, OpKind, StructureKind, Terminator as MirTerminator, ValueId};

type TestResult<T> = Result<T, Box<dyn Error>>;

fn context(form_index: u32, source: &str) -> TestResult<(FormId, HirFormBuilder, SymbolTable)> {
    let compilation_unit = CompilationUnitId::new(0);
    let form = FormId::new(compilation_unit, form_index);
    let mut files = SourceFiles::new(compilation_unit);
    let file = files.add("lowering-handlers.lisp", source)?;
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

#[test]
fn throw_value_reaches_the_matching_catch_result() -> TestResult<()> {
    let source = "(catch 77 (throw 77 41))";
    let (_form_id, mut builder, symbols) = context(0, source)?;
    let scope = builder.root_scope();
    let tag = literal(
        &mut builder,
        scope,
        Literal::Number(NumberLiteral::Fixnum(77)),
    )?;
    let thrown_value = literal(
        &mut builder,
        scope,
        Literal::Number(NumberLiteral::Fixnum(41)),
    )?;
    let throw = builder.alloc_node(
        scope,
        NodeKind::Throw {
            tag,
            value: thrown_value,
        },
    )?;
    let catch = builder.alloc_node(
        scope,
        NodeKind::Catch {
            tag,
            body: vec![throw],
        },
    )?;
    builder.add_root(catch)?;

    let form = builder.finish()?;
    let lowered = ncl_hir::lower::lower(&form, &symbols)?;

    // CLHS specifies that THROW transfers to the most recently established matching CATCH,
    // which returns the result form's values. This case carries one value.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_throw.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_catch.htm
    let region = lowered
        .entry
        .handler_regions
        .iter()
        .find(|region| region.kind == HandlerKind::Catch)
        .ok_or_else(|| std::io::Error::other("CATCH handler region is missing"))?;
    let (throw_block, thrown_tag, thrown_value, throw_condition, builtin_result) = lowered
        .entry
        .blocks
        .iter()
        .find_map(|block| {
            let MirTerminator::Throw { condition } = &block.terminator else {
                return None;
            };
            let operation = block.ops.iter().find(|operation| {
                matches!(&operation.kind, OpKind::Builtin { name, .. } if name == "throw")
            })?;
            let OpKind::Builtin { args, .. } = &operation.kind else {
                return None;
            };
            Some((
                block,
                *args.first()?,
                *args.get(1)?,
                *condition,
                operation.results.first()?.0,
            ))
        })
        .ok_or_else(|| std::io::Error::other("THROW terminator and operation are missing"))?;
    assert_eq!(throw_condition, builtin_result);
    assert!(region.protected.contains(&throw_block.id));
    assert_eq!(region.catch_tag, Some(thrown_tag));
    assert_eq!(
        constant_for_value(&lowered.entry, thrown_tag),
        Some(&Constant::Fixnum(77))
    );
    assert_eq!(
        constant_for_value(&lowered.entry, thrown_value),
        Some(&Constant::Fixnum(41))
    );

    let handler = lowered
        .entry
        .blocks
        .iter()
        .find(|block| block.id == region.handler)
        .ok_or_else(|| std::io::Error::other("CATCH result handler is missing"))?;
    let caught_value = handler
        .params
        .first()
        .map(|parameter| parameter.value)
        .ok_or_else(|| std::io::Error::other("CATCH result parameter is missing"))?;
    assert!(handler.ops.iter().any(|operation| {
        matches!(&operation.kind, OpKind::LeaveHandler { region: id } if *id == region.id)
    }));
    let MirTerminator::Jump { target, args } = &handler.terminator else {
        return Err(std::io::Error::other("CATCH result transfer is missing").into());
    };
    assert_eq!(args.first(), Some(&caught_value));
    let result_block = lowered
        .entry
        .blocks
        .iter()
        .find(|block| block.id == *target)
        .ok_or_else(|| std::io::Error::other("CATCH result block is missing"))?;
    let result = result_block
        .params
        .first()
        .map(|parameter| parameter.value)
        .ok_or_else(|| std::io::Error::other("CATCH merge parameter is missing"))?;
    assert!(matches!(
        &result_block.terminator,
        MirTerminator::Return { values } if values.first() == Some(&result)
    ));
    Ok(())
}

#[test]
fn progv_normal_exit_leaves_its_dynamic_binding_and_returns_the_body_value() -> TestResult<()> {
    let source = "(progv '(special) '(5) 29)";
    let (form_id, mut builder, mut symbols) = context(1, source)?;
    let scope = builder.root_scope();
    let special = intern_symbol(
        form_id,
        &mut builder,
        &mut symbols,
        "COMMON-LISP-USER",
        "SPECIAL",
    )?;
    let symbols_list = literal(
        &mut builder,
        scope,
        Literal::Cons(Box::new(Literal::Symbol(special)), Box::new(Literal::Nil)),
    )?;
    let values_list = literal(
        &mut builder,
        scope,
        Literal::Cons(
            Box::new(Literal::Number(NumberLiteral::Fixnum(5))),
            Box::new(Literal::Nil),
        ),
    )?;
    let body_value = literal(
        &mut builder,
        scope,
        Literal::Number(NumberLiteral::Fixnum(29)),
    )?;
    let progv = builder.alloc_node(
        scope,
        NodeKind::DynamicBinding {
            symbols: symbols_list,
            values: values_list,
            body: vec![body_value],
        },
    )?;
    builder.add_root(progv)?;

    let form = builder.finish()?;
    let lowered = ncl_hir::lower::lower(&form, &symbols)?;

    // CLHS specifies that PROGV bindings are undone when its body forms exit normally.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_progv.htm
    let region = lowered
        .entry
        .handler_regions
        .iter()
        .find(|region| region.kind == HandlerKind::Progv)
        .ok_or_else(|| std::io::Error::other("PROGV handler region is missing"))?;
    assert_eq!(region.binding_targets.len(), 2);
    let Some(Constant::Structure {
        kind: StructureKind::Cons,
        elements: symbol_elements,
    }) = constant_for_value(&lowered.entry, region.binding_targets[0])
    else {
        return Err(std::io::Error::other("PROGV symbol sequence is not a list constant").into());
    };
    assert_eq!(symbol_elements.len(), 2);
    assert_eq!(
        constant_at(&lowered.entry, symbol_elements[0]),
        Some(&Constant::Symbol {
            package: "COMMON-LISP-USER".to_owned(),
            name: "SPECIAL".to_owned(),
        })
    );
    assert_eq!(
        constant_at(&lowered.entry, symbol_elements[1]),
        Some(&Constant::Nil)
    );
    let Some(Constant::Structure {
        kind: StructureKind::Cons,
        elements: value_elements,
    }) = constant_for_value(&lowered.entry, region.binding_targets[1])
    else {
        return Err(std::io::Error::other("PROGV value sequence is not a list constant").into());
    };
    assert_eq!(value_elements.len(), 2);
    assert_eq!(
        constant_at(&lowered.entry, value_elements[0]),
        Some(&Constant::Fixnum(5))
    );
    assert_eq!(
        constant_at(&lowered.entry, value_elements[1]),
        Some(&Constant::Nil)
    );

    let expected_body_value = Constant::Fixnum(29);
    let transfer = lowered
        .entry
        .blocks
        .iter()
        .find(|block| {
            region.protected.contains(&block.id)
                && block.ops.iter().any(|operation| {
                    matches!(&operation.kind, OpKind::LeaveHandler { region: id } if *id == region.id)
                })
                && matches!(
                    &block.terminator,
                    MirTerminator::Jump { args, .. }
                        if args.first().is_some_and(|value| {
                            constant_for_value(&lowered.entry, *value) == Some(&expected_body_value)
                        })
                )
        })
        .ok_or_else(|| std::io::Error::other("PROGV normal exit does not transfer its body value"))?;
    let MirTerminator::Jump { target, .. } = &transfer.terminator else {
        return Err(std::io::Error::other("PROGV merge transfer is missing").into());
    };
    let merge = lowered
        .entry
        .blocks
        .iter()
        .find(|block| block.id == *target)
        .ok_or_else(|| std::io::Error::other("PROGV merge block is missing"))?;
    let result = merge
        .params
        .first()
        .map(|parameter| parameter.value)
        .ok_or_else(|| std::io::Error::other("PROGV result parameter is missing"))?;
    assert!(matches!(
        &merge.terminator,
        MirTerminator::Return { values } if values.first() == Some(&result)
    ));
    Ok(())
}

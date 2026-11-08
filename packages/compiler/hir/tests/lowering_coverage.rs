//! Behavior checks for less common verified HIR-to-MIR lowering paths.

use std::error::Error;

use ncl_hir::{
    CompilationUnitId, FormId, HirFormBuilder, Literal, MultipleValueOperation, NodeId, NodeKind,
    NumberLiteral, SourceFiles, SourceOrigins, SymbolTable,
};
use ncl_ir::{Constant, ConstantIndex, Function, OpKind, StructureKind, Terminator, ValueId};

type TestResult<T> = Result<T, Box<dyn Error>>;

fn context(form_index: u32, source: &str) -> TestResult<(FormId, HirFormBuilder, SymbolTable)> {
    let unit = CompilationUnitId::new(45);
    let form = FormId::new(unit, form_index);
    let mut files = SourceFiles::new(unit);
    let file = files.add("lowering-coverage.lisp", source)?;
    let span = files.span(file, 0, source.len())?;
    let mut origins = SourceOrigins::new(form);
    let origin = origins.add_source(&files, file, span)?;
    let builder = HirFormBuilder::new(form, origins, origin)?;
    Ok((form, builder, SymbolTable::new(unit)))
}

fn returned_value(function: &Function) -> Option<ValueId> {
    function
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            Terminator::Return { values } if values.len() == 1 => values.first().copied(),
            _ => None,
        })
}

fn constant_at(function: &Function, index: ConstantIndex) -> Option<&Constant> {
    function.constants.get(usize::try_from(index.0).ok()?)
}

fn constant_for_value(function: &Function, value: ValueId) -> Option<&Constant> {
    let operation = function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find(|operation| operation.results.iter().any(|(result, _)| *result == value))?;
    match &operation.kind {
        OpKind::Const { result } => constant_at(function, *result),
        OpKind::Convert { value, .. } | OpKind::Move { value } => {
            constant_for_value(function, *value)
        }
        _ => None,
    }
}

fn verify(function: &Function) -> TestResult<()> {
    ncl_ir::verify(function)
        .map_err(|errors| std::io::Error::other(format!("MIR verification failed: {errors:?}")))?;
    Ok(())
}

fn assert_unrepresentable(
    form_index: u32,
    source: &str,
    symbol_package: &'static str,
    symbol_name: &'static str,
    expected_operation: &'static str,
    make_node: impl FnOnce(ncl_hir::SymbolId, NodeId) -> NodeKind,
) -> TestResult<()> {
    let (form_id, mut builder, mut symbols) = context(form_index, source)?;
    let scope = builder.root_scope();
    let symbol = symbols.intern(form_id, Some(symbol_package), symbol_name, None)?;
    builder.register_symbol(symbol)?;
    let value = builder.alloc_node(
        scope,
        NodeKind::Literal(Literal::Number(NumberLiteral::Fixnum(5))),
    )?;
    let node = builder.alloc_node(scope, make_node(symbol, value))?;
    builder.add_root(node)?;
    let form = builder.finish()?;

    assert!(matches!(
        ncl_hir::lower::lower(&form, &symbols),
        Err(ncl_hir::lower::LowerError::Unrepresentable {
            node: found_node,
            operation,
        }) if found_node == node && operation == expected_operation
    ));
    Ok(())
}

#[test]
fn read_only_load_time_vector_keeps_its_ordered_elements() -> TestResult<()> {
    let (form_id, mut builder, symbols) = context(0, "(load-time-value #(2 7) t)")?;
    let scope = builder.root_scope();
    let vector = builder.alloc_node(
        scope,
        NodeKind::Literal(Literal::Vector(vec![
            Literal::Number(NumberLiteral::Fixnum(2)),
            Literal::Number(NumberLiteral::Fixnum(7)),
        ])),
    )?;
    let load_time_value = builder.alloc_node(
        scope,
        NodeKind::LoadTimeValue {
            form: vector,
            read_only: true,
        },
    )?;
    builder.add_root(load_time_value)?;
    let form = builder.finish()?;
    assert_eq!(form.id(), form_id);

    let lowered = ncl_hir::lower::lower(&form, &symbols)?;
    verify(&lowered.entry)?;
    let returned = returned_value(&lowered.entry)
        .ok_or_else(|| std::io::Error::other("vector result is not returned"))?;
    let Some(Constant::Structure {
        kind: StructureKind::SimpleVector,
        elements,
    }) = constant_for_value(&lowered.entry, returned)
    else {
        return Err(
            std::io::Error::other("read-only vector is not a simple-vector constant").into(),
        );
    };
    let first = elements
        .first()
        .and_then(|index| constant_at(&lowered.entry, *index))
        .ok_or_else(|| std::io::Error::other("first vector element is missing"))?;
    let second = elements
        .get(1)
        .and_then(|index| constant_at(&lowered.entry, *index))
        .ok_or_else(|| std::io::Error::other("second vector element is missing"))?;
    assert_eq!(elements.len(), 2);
    assert_eq!(first, &Constant::Fixnum(2));
    assert_eq!(second, &Constant::Fixnum(7));
    // CLHS LOAD-TIME-VALUE treats a read-only result as a literal object; vector element order is retained.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_ld_tim.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_smp_ve.htm
    Ok(())
}

#[test]
fn zero_value_production_keeps_zero_arity_and_a_nil_primary_value() -> TestResult<()> {
    let (form_id, mut builder, symbols) = context(1, "(values)")?;
    let root = builder.alloc_node(
        builder.root_scope(),
        NodeKind::MultipleValues(MultipleValueOperation::Produce { values: Vec::new() }),
    )?;
    builder.add_root(root)?;
    let form = builder.finish()?;

    let lowered = ncl_hir::lower::lower(&form, &symbols)?;
    verify(&lowered.entry)?;
    assert!(lowered.entry.blocks.iter().flat_map(|block| &block.ops).any(
        |operation| matches!(&operation.kind, OpKind::SetMultipleValues { values } if values.is_empty())
    ));
    let returned = returned_value(&lowered.entry)
        .ok_or_else(|| std::io::Error::other("primary result is not returned"))?;
    assert_eq!(
        constant_for_value(&lowered.entry, returned),
        Some(&Constant::Nil)
    );
    assert_eq!(form.id(), form_id);
    // CLHS VALUES returns exactly the objects supplied, including zero objects.
    // https://www.lispworks.com/documentation/HyperSpec/Body/f_values.htm
    Ok(())
}

#[test]
fn execute_situation_lowers_the_eval_when_body_value() -> TestResult<()> {
    let (form_id, mut builder, symbols) = context(2, "(eval-when (:execute) 29)")?;
    let scope = builder.root_scope();
    let body = builder.alloc_node(
        scope,
        NodeKind::Literal(Literal::Number(NumberLiteral::Fixnum(29))),
    )?;
    let eval_when = builder.alloc_node(
        scope,
        NodeKind::CompileTimeForm {
            situations: vec![ncl_hir::CompileTimeSituation::Execute],
            form: body,
        },
    )?;
    builder.add_root(eval_when)?;
    let form = builder.finish()?;

    let lowered = ncl_hir::lower::lower(&form, &symbols)?;
    verify(&lowered.entry)?;
    let returned = returned_value(&lowered.entry)
        .ok_or_else(|| std::io::Error::other("EVAL-WHEN body value is not returned"))?;
    assert_eq!(
        constant_for_value(&lowered.entry, returned),
        Some(&Constant::Fixnum(29))
    );
    assert_eq!(form.id(), form_id);
    // CLHS says :EXECUTE processes the body as an implicit PROGN outside top-level compilation.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_eval_w.htm
    Ok(())
}

#[test]
fn unsupported_hir_operations_fail_closed_with_their_operation_identity() -> TestResult<()> {
    // HIR keeps these operations distinct until a transform can preserve their meaning.
    // Rejecting them avoids replacing their identity with a guessed MIR call.
    // docs/src/design/hir.md:63-78
    assert_unrepresentable(
        3,
        "(car 5)",
        "COMMON-LISP",
        "CAR",
        "known-function-operation",
        |symbol, value| NodeKind::KnownFunctionOperation {
            function: symbol,
            arguments: vec![value],
        },
    )?;
    assert_unrepresentable(
        4,
        "(elt 5 0)",
        "COMMON-LISP",
        "ELT",
        "sequence-operation",
        |symbol, value| NodeKind::SequenceOperation {
            function: symbol,
            arguments: vec![value],
        },
    )?;
    assert_unrepresentable(
        5,
        "(aref 5 0)",
        "COMMON-LISP",
        "AREF",
        "array-operation",
        |symbol, value| NodeKind::ArrayOperation {
            function: symbol,
            arguments: vec![value],
        },
    )?;
    assert_unrepresentable(
        7,
        "(defun f () 5)",
        "COMMON-LISP-USER",
        "F",
        "top-level-definition",
        |symbol, value| NodeKind::TopLevelDefinition {
            name: symbol,
            kind: ncl_hir::DefinitionKind::Function,
            value,
        },
    )?;
    Ok(())
}

#[test]
fn runtime_check_fails_closed_when_used_as_a_control_flow_condition() -> TestResult<()> {
    let (form_id, mut builder, symbols) = context(8, "(runtime-check function value)")?;
    let scope = builder.root_scope();
    let value = builder.alloc_node(
        scope,
        NodeKind::Literal(Literal::Number(NumberLiteral::Fixnum(5))),
    )?;
    let check = builder.alloc_node(
        scope,
        NodeKind::RuntimeCheck {
            kind: ncl_hir::RuntimeCheckKind::Function,
            value,
        },
    )?;
    let tagbody = builder.alloc_node(
        scope,
        NodeKind::Tagbody {
            tags: Vec::new(),
            forms: Vec::new(),
        },
    )?;
    let flow = builder.control_flow_mut();
    flow.begin_function(tagbody)?;
    let entry = flow.create_block(tagbody)?;
    let accepted = flow.create_block(tagbody)?;
    let rejected = flow.create_block(tagbody)?;
    flow.set_terminator(
        entry,
        ncl_hir::Terminator::Conditional {
            condition: check,
            if_true: accepted,
            if_false: rejected,
        },
    )?;
    flow.set_terminator(accepted, ncl_hir::Terminator::Return { values: Vec::new() })?;
    flow.set_terminator(rejected, ncl_hir::Terminator::Return { values: Vec::new() })?;
    builder.add_root(tagbody)?;
    let form = builder.finish()?;

    assert!(matches!(
        ncl_hir::lower::lower(&form, &symbols),
        Err(ncl_hir::lower::LowerError::Unrepresentable {
            node,
            operation: "runtime-check",
        }) if node == check
    ));
    assert_eq!(form.id(), form_id);
    Ok(())
}

use std::error::Error;

use crate::forms::Fixture;
use ncl_compiler_front::Expr;
use ncl_compiler_front::hir_builder::HirBuilder;
use ncl_hir::{FormId, HirForm, NodeKind, SourceFiles, SourceOriginId, SourceOrigins};
use ncl_ir::{Compare, Constant, OpKind, Terminator, ValueId};
use ncl_object::Word;

pub(super) type TestResult<T> = Result<T, Box<dyn Error>>;

pub(super) fn source_origin(
    form: FormId,
    source: &str,
) -> TestResult<(SourceOrigins, SourceOriginId)> {
    let mut files = SourceFiles::new(form.compilation_unit());
    let file = files.add("hir-bridge.lisp", source)?;
    let span = files.span(file, 0, source.len())?;
    let mut origins = SourceOrigins::new(form);
    let origin = origins.add_source(&files, file, span)?;
    Ok((origins, origin))
}

pub(super) fn fixture_text(source: &'static str, expected: &str) -> &'static str {
    // The test fixture helper builds `Word`s directly, so the text is an origin
    // and this exact comparison guards it against drifting from the AST setup.
    assert_eq!(source, expected);
    source
}

pub(super) fn context(unit: u32) -> (FormId, HirBuilder, Fixture) {
    let id = FormId::new(ncl_hir::CompilationUnitId::new(unit), 0);
    (id, HirBuilder::new(id.compilation_unit()), Fixture::new())
}

pub(super) fn build(
    builder: &mut HirBuilder,
    id: FormId,
    expr: &Expr,
    source: &str,
) -> TestResult<HirForm> {
    let (origins, origin) = source_origin(id, source)?;
    let form = builder.build_form(id, expr, origins, origin)?;
    form.verify(builder.symbols())?;
    assert!(form.nodes().iter().all(|(_, node)| node.origin == origin));
    Ok(form)
}

pub(super) fn expand_build(
    builder: &mut HirBuilder,
    fixture: &mut Fixture,
    id: FormId,
    input: Word,
    source: &str,
) -> TestResult<(Expr, HirForm)> {
    let expr = fixture.expand(input)?;
    let form = build(builder, id, &expr, source)?;
    Ok((expr, form))
}

pub(super) fn has_node(form: &HirForm, test: impl Fn(&NodeKind) -> bool) -> bool {
    form.nodes().iter().any(|(_, node)| test(&node.kind))
}

pub(super) fn lower_both_and_verify(
    builder: &HirBuilder,
    form: &HirForm,
    expr: &Expr,
) -> TestResult<(ncl_hir::lower::Lowered, ncl_compiler_front::Lowered)> {
    let hir = ncl_hir::lower::lower(form, builder.symbols())?;
    let baseline = ncl_compiler_front::lower::lower_toplevel(expr)?;
    verify_ir(&hir.entry, "verify HIR MIR")?;
    for function in &hir.nested {
        verify_ir(function, "verify nested HIR MIR")?;
    }
    verify_ir(&baseline.entry, "verify baseline MIR")?;
    for function in &baseline.nested {
        verify_ir(function, "verify nested baseline MIR")?;
    }
    Ok((hir, baseline))
}

pub(super) fn verify_ir(function: &ncl_ir::Function, label: &str) -> TestResult<()> {
    ncl_ir::verify(function)
        .map_err(|errors| std::io::Error::other(format!("{label}: {errors:?}")))?;
    Ok(())
}

pub(super) fn constant_conditional_signature(
    function: &ncl_ir::Function,
) -> Option<(bool, i64, i64)> {
    let (condition, then_target, else_target) =
        function
            .blocks
            .iter()
            .find_map(|block| match &block.terminator {
                Terminator::Branch {
                    condition,
                    then_target,
                    else_target,
                    ..
                } => Some((*condition, *then_target, *else_target)),
                _ => None,
            })?;
    let (operation, left, right) = function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|op| {
            if op.results.iter().any(|(value, _)| *value == condition)
                && let OpKind::Compare { op, left, right } = &op.kind
            {
                Some((*op, *left, *right))
            } else {
                None
            }
        })?;
    let left = constant_for_value(function, left)?;
    let right = constant_for_value(function, right)?;
    let condition = match operation {
        Compare::Eq => left == right,
        Compare::Ne => left != right,
        _ => return None,
    };
    Some((
        condition,
        fixnum_constant_in_block(function, then_target)?,
        fixnum_constant_in_block(function, else_target)?,
    ))
}

fn constant_for_value(function: &ncl_ir::Function, value: ValueId) -> Option<&Constant> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|op| {
            if op.results.iter().any(|(result, _)| *result == value)
                && let OpKind::Const { result } = &op.kind
            {
                function.constants.get(usize::try_from(result.0).ok()?)
            } else {
                None
            }
        })
}

fn fixnum_constant_in_block(function: &ncl_ir::Function, id: ncl_ir::BlockId) -> Option<i64> {
    let block = function.blocks.iter().find(|block| block.id == id)?;
    block.ops.iter().find_map(|op| {
        let OpKind::Const { result } = &op.kind else {
            return None;
        };
        match function.constants.get(usize::try_from(result.0).ok()?)? {
            Constant::Fixnum(value) => Some(*value),
            _ => None,
        }
    })
}

use crate::forms::Fixture;
use ncl_compiler_front::Expr;
use ncl_compiler_front::hir_builder::HirBuilder;
use ncl_hir::{FormId, HirForm, NodeKind, SourceFiles, SourceOriginId, SourceOrigins};
use ncl_ir::{Compare, Constant, OpKind, Terminator, ValueId};
use ncl_object::Word;

pub(super) fn source_origin(form: FormId, source: &str) -> (SourceOrigins, SourceOriginId) {
    let mut files = SourceFiles::new(form.compilation_unit());
    let file = files.add("hir-bridge.lisp", source).expect("source file");
    let span = files.span(file, 0, source.len()).expect("source span");
    let mut origins = SourceOrigins::new(form);
    let origin = origins
        .add_source(&files, file, span)
        .expect("source origin");
    (origins, origin)
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

pub(super) fn build(builder: &mut HirBuilder, id: FormId, expr: &Expr, source: &str) -> HirForm {
    let (origins, origin) = source_origin(id, source);
    let form = builder
        .build_form(id, expr, origins, origin)
        .expect("build HIR");
    form.verify(builder.symbols()).expect("verify HIR");
    assert!(form.nodes().iter().all(|(_, node)| node.origin == origin));
    form
}

pub(super) fn expand_build(
    builder: &mut HirBuilder,
    fixture: &mut Fixture,
    id: FormId,
    input: Word,
    source: &str,
) -> (Expr, HirForm) {
    let expr = fixture.expand(input).expect("expand bridge fixture");
    let form = build(builder, id, &expr, source);
    (expr, form)
}

pub(super) fn has_node(form: &HirForm, test: impl Fn(&NodeKind) -> bool) -> bool {
    form.nodes().iter().any(|(_, node)| test(&node.kind))
}

pub(super) fn lower_both_and_verify(
    builder: &HirBuilder,
    form: &HirForm,
    expr: &Expr,
) -> (ncl_hir::lower::Lowered, ncl_compiler_front::Lowered) {
    let hir = ncl_hir::lower::lower(form, builder.symbols()).expect("lower HIR");
    let baseline = ncl_compiler_front::lower::lower_toplevel(expr).expect("lower baseline AST");
    ncl_ir::verify(&hir.entry).expect("verify HIR MIR");
    for function in &hir.nested {
        ncl_ir::verify(function).expect("verify nested HIR MIR");
    }
    ncl_ir::verify(&baseline.entry).expect("verify baseline MIR");
    for function in &baseline.nested {
        ncl_ir::verify(function).expect("verify nested baseline MIR");
    }
    (hir, baseline)
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

//! Public form construction exercises verifier paths missed by the existing fixtures.

#![deny(
    unreachable_pub,
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::as_conversions
)]

use std::collections::{HashMap, HashSet};
use std::error::Error;

use ncl_hir::binding::LocalFunctionKind;
use ncl_hir::cl_type::BuiltinType;
use ncl_hir::control::{ControlError, ControlFlowBuilder, ExitTarget, Terminator};
use ncl_hir::node::{
    CallTarget, CompileTimeSituation, DefinitionKind, LetBinding, LetKind, LiteralElementType,
    LocalFunctionDefinition, MultipleValueOperation, NodeKind, TagbodyTag, VariableTarget,
};
use ncl_hir::verify::{FactEpochs, VerifyInput, VerifyResources, verify};
use ncl_hir::{
    CleanupId, CompilationUnitId, FormId, HirForm, HirFormBuilder, IdError, LambdaKey,
    LambdaOptional, LambdaSignature, Literal, SourceFiles, SourceOriginId, SourceOrigins, SymbolId,
    SymbolTable, TypeId, TypeSpecConstId, ValueId, VerifyError,
};

fn form_builder(form: FormId) -> Result<HirFormBuilder, Box<dyn Error>> {
    let mut files = SourceFiles::new(form.compilation_unit());
    let file = files.add("verifier-coverage-followup.lisp", "x")?;
    let span = files.span(file, 0, 1)?;
    let mut origins = SourceOrigins::new(form);
    let origin = origins.add_source(&files, file, span)?;
    Ok(HirFormBuilder::new(form, origins, origin)?)
}

fn symbol_table(form: FormId, names: &[&str]) -> Result<SymbolTable, Box<dyn Error>> {
    let mut symbols = SymbolTable::new(form.compilation_unit());
    for name in names {
        symbols.intern(form, Some("CL"), name, None)?;
    }
    Ok(symbols)
}

fn verify_with_resources(
    hir: &HirForm,
    form: FormId,
    resources: VerifyResources<'_>,
) -> Result<(), VerifyError> {
    verify(VerifyInput {
        form,
        nodes: hir.nodes(),
        types: hir.types(),
        bindings: hir.bindings(),
        control_flow: hir.control_flow(),
        origins: hir.origins(),
        facts: hir.facts(),
        epochs: FactEpochs::default(),
        policy: hir.policy(),
        resources,
    })
}

const fn empty_lambda_signature() -> LambdaSignature {
    LambdaSignature {
        required: Vec::new(),
        optional: Vec::new(),
        rest: None,
        keys: Vec::new(),
        allow_other_keys: false,
        aux: Vec::new(),
    }
}

#[path = "verifier_coverage_followup/control.rs"]
mod control;
#[path = "verifier_coverage_followup/form_contracts.rs"]
mod form_contracts;
#[path = "verifier_coverage_followup/references.rs"]
mod references;
#[path = "verifier_coverage_followup/resources.rs"]
mod resources;
#[path = "verifier_coverage_followup/structure.rs"]
mod structure;
#[path = "verifier_coverage_followup/type_checks.rs"]
mod type_checks;

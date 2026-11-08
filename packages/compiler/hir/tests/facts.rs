//! Cached fact metadata, pass invalidation, and verifier build defaults.

use std::error::Error;

use ncl_hir::verify::{
    FactAuthorization, FactCertainty, FactDomain, FactEpochs, FactProvenance, FactRecord,
    FactStore, PassInvalidations, VerificationBuild, VerificationConfig,
};
use ncl_hir::{CompilationUnitId, FormId, SourceOriginId};

#[test]
fn fact_store_preserves_metadata_and_rejects_ids_from_another_form() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(13), 0);
    let origin = SourceOriginId::try_from_index(form, 2)?;
    let record = FactRecord::new(
        FactDomain::Escape,
        4,
        origin,
        FactProvenance::Inferred,
        FactCertainty::Proven,
        FactAuthorization::Informational,
    );
    assert_eq!(record.domain(), FactDomain::Escape);
    assert_eq!(record.epoch(), 4);
    assert_eq!(record.source_origin(), origin);
    assert_eq!(record.provenance(), FactProvenance::Inferred);
    assert_eq!(record.certainty(), FactCertainty::Proven);
    assert_eq!(record.authorization(), FactAuthorization::Informational);

    let mut store = FactStore::new(form);
    let id = store.add(record)?;
    assert_eq!(id.form(), form);
    assert_eq!(id.index(), 0);
    assert_eq!(store.get(id), Some(&record));

    let other_form = FormId::new(form.compilation_unit(), 1);
    let other_store = FactStore::new(other_form);
    assert_eq!(other_store.get(id), None);
    Ok(())
}

#[test]
fn pass_invalidations_advance_each_declared_domain_once() -> Result<(), Box<dyn Error>> {
    let epochs = FactEpochs::default();
    for domain in [
        FactDomain::Type,
        FactDomain::Effect,
        FactDomain::Escape,
        FactDomain::Dominance,
        FactDomain::ControlFlow,
    ] {
        assert_eq!(epochs.current(domain), 0);
    }

    let next = epochs.after_pass(&PassInvalidations::new([
        FactDomain::Type,
        FactDomain::Type,
        FactDomain::Effect,
        FactDomain::Escape,
        FactDomain::Dominance,
        FactDomain::ControlFlow,
    ]))?;
    for domain in [
        FactDomain::Type,
        FactDomain::Effect,
        FactDomain::Escape,
        FactDomain::Dominance,
        FactDomain::ControlFlow,
    ] {
        assert_eq!(next.current(domain), 1);
    }

    let type_only = next.after_pass(&PassInvalidations::new([FactDomain::Type]))?;
    assert_eq!(type_only.current(FactDomain::Type), 2);
    assert_eq!(type_only.current(FactDomain::Effect), 1);
    assert_eq!(type_only.current(FactDomain::Escape), 1);
    assert_eq!(type_only.current(FactDomain::Dominance), 1);
    assert_eq!(type_only.current(FactDomain::ControlFlow), 1);
    Ok(())
}

#[test]
fn verifier_build_defaults_keep_tests_and_ci_enabled_and_gate_release() {
    for build in [VerificationBuild::Test, VerificationBuild::Ci] {
        for gates in [(false, false), (false, true), (true, false), (true, true)] {
            assert!(VerificationConfig::new(build, gates.0, gates.1, false).enabled());
            assert!(VerificationConfig::new(build, gates.0, gates.1, true).enabled());
        }
    }

    assert!(VerificationConfig::new(VerificationBuild::Release, false, false, false).enabled());
    assert!(VerificationConfig::new(VerificationBuild::Release, true, false, false).enabled());
    assert!(VerificationConfig::new(VerificationBuild::Release, false, true, false).enabled());
    assert!(!VerificationConfig::new(VerificationBuild::Release, true, true, false).enabled());
    assert!(VerificationConfig::new(VerificationBuild::Release, true, true, true).enabled());
}

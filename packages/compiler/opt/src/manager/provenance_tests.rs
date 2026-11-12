use super::{Module, PassManager, PassManagerOptions, RenamingPass, address_leaf, record_events};
use crate::PassVerification;
use crate::tests_support::Fixture;
use ncl_ir::{AddressLivenessBoundary, Op, OpKind, VerifyError};

#[test]
fn manager_keeps_a_preexisting_address_liveness_error_when_a_pass_renames_the_function() {
    let mut original = address_leaf();
    original.blocks[0].ops.insert(
        0,
        Op {
            results: Vec::new(),
            kind: OpKind::Safepoint,
            loc: None,
        },
    );
    let Err(existing_errors) = ncl_ir::verify(&original) else {
        std::process::exit(1);
    };
    assert!(existing_errors.iter().any(|error| matches!(
        error,
        VerifyError::AddressLiveAcross {
            boundary: AddressLivenessBoundary::Safepoint,
            ..
        }
    )));

    let mut manager = PassManager::with_options(PassManagerOptions {
        max_iterations: 1,
        verify_after_each_pass: false,
    });
    manager.add_function_pass(RenamingPass);
    let mut module = Module {
        functions: vec![original],
    };
    let mut events = Vec::new();
    let report = manager
        .run_with_observer(&mut module, record_events(&mut events))
        .fixture();

    assert_eq!(module.functions[0].name, "address-leaf-renamed");
    assert_eq!(report.stats.len(), 1);
    assert_eq!(report.stats[0].pass, "rename");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].changed, Some(true));
    assert_eq!(events[0].verification, PassVerification::Skipped);
    assert_eq!(events[0].error, None);
}

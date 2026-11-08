use super::*;

#[derive(Debug)]
struct FailingModulePass;

impl ModulePass for FailingModulePass {
    fn name(&self) -> &'static str {
        "module-failing"
    }

    fn run(&mut self, _module: &mut Module) -> PassResult {
        Err(PassError::new("inner", "module rejected"))
    }
}

#[derive(Debug)]
struct FailingProvenanceInvalidatingModulePass;

impl ModulePass for FailingProvenanceInvalidatingModulePass {
    fn name(&self) -> &'static str {
        "failing-provenance-invalidating-module"
    }

    fn run(&mut self, module: &mut Module) -> PassResult {
        module.functions[0].blocks[0].ops.insert(
            0,
            Op {
                results: Vec::new(),
                kind: OpKind::Safepoint,
                loc: None,
            },
        );
        Err(PassError::new("inner", "rejected address liveness"))
    }
}

#[test]
fn module_pass_error_without_provenance_keeps_module_and_reports_failure() {
    let original = Module {
        functions: vec![leaf()],
    };
    let mut module = original.clone();
    let mut manager = PassManager::new();
    manager.add_module_pass(FailingModulePass);
    let mut events = Vec::new();

    let error = manager
        .run_with_observer(&mut module, |event| events.push(event.clone()))
        .err()
        .ok_or("module pass error should be returned")
        .fixture();

    assert_eq!(error.pass, "module-failing");
    assert_eq!(error.message, "module rejected");
    assert_eq!(module.functions, original.functions);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].pass, "module-failing");
    assert_eq!(events[0].function, None);
    assert_eq!(events[0].changed, None);
    assert_eq!(events[0].verification, crate::PassVerification::NotRun);
    assert_eq!(events[0].error.as_deref(), Some("module rejected"));
}

#[test]
fn module_pass_error_after_address_invalidation_rolls_back_and_reports_provenance() {
    let original_function = address_leaf();
    assert!(ncl_ir::verify(&original_function).is_ok());
    let original = Module {
        functions: vec![original_function],
    };
    let mut module = original.clone();
    let mut manager = PassManager::new();
    manager.add_module_pass(FailingProvenanceInvalidatingModulePass);
    let mut events = Vec::new();

    let report = manager
        .run_with_observer(&mut module, |event| events.push(event.clone()))
        .fixture();

    assert_eq!(module.functions, original.functions);
    assert_eq!(report.stats.len(), 1);
    assert_eq!(
        report.stats[0].pass,
        "failing-provenance-invalidating-module"
    );
    assert!(!report.stats[0].changed);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].pass, "failing-provenance-invalidating-module");
    assert_eq!(events[0].function, None);
    assert_eq!(events[0].changed, Some(false));
    assert!(matches!(
        &events[0].verification,
        crate::PassVerification::RolledBack(message)
            if message.contains("AddressLiveAcross")
                && message.contains("pass error: rejected address liveness")
    ));
    assert!(
        events[0]
            .error
            .as_deref()
            .is_some_and(|message| message.contains("AddressLiveAcross"))
    );
}

#[test]
fn unchanged_module_pass_reports_successful_verification() {
    let original = Module {
        functions: vec![leaf()],
    };
    let mut module = original.clone();
    let mut manager = PassManager::new();
    manager.add_module_pass(ModuleToggle);
    let mut events = Vec::new();

    let report = manager
        .run_with_observer(&mut module, |event| events.push(event.clone()))
        .fixture();

    assert_eq!(module.functions, original.functions);
    assert_eq!(report.stats.len(), 1);
    assert_eq!(report.stats[0].pass, "module-toggle");
    assert!(!report.stats[0].changed);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].pass, "module-toggle");
    assert_eq!(events[0].changed, Some(false));
    assert_eq!(events[0].verification, crate::PassVerification::Passed);
    assert!(events[0].error.is_none());
}

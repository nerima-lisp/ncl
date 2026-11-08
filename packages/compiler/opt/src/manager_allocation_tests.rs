use crate::tests_support::{Fixture, leaf};
use crate::{FunctionPass, Module, ModulePass, PassManager, PassManagerOptions, PassResult};
use ncl_ir::Function;

#[derive(Debug)]
struct UnchangedPass;

impl FunctionPass for UnchangedPass {
    fn name(&self) -> &'static str {
        "unchanged"
    }

    fn run(&mut self, _function: &mut Function, _module: &Module) -> PassResult {
        Ok(false)
    }
}

#[derive(Debug)]
struct UnchangedModulePass;

impl ModulePass for UnchangedModulePass {
    fn name(&self) -> &'static str {
        "unchanged-module"
    }

    fn run(&mut self, _module: &mut Module) -> PassResult {
        Ok(false)
    }
}

#[test]
fn allocation_snapshot_reports_a_valid_delta_and_omits_decreasing_values() {
    let mut manager = PassManager::with_options(PassManagerOptions {
        max_iterations: 1,
        verify_after_each_pass: true,
    });
    manager.add_function_pass(UnchangedPass);
    manager.add_module_pass(UnchangedModulePass);
    let mut snapshots = [Some(17_u128), Some(61_u128), Some(92_u128), Some(100_u128)].into_iter();
    let mut events = Vec::new();
    let report = manager
        .run_with_observer_and_allocation_snapshot(
            &mut Module {
                functions: vec![leaf()],
            },
            |event| events.push(event.clone()),
            || snapshots.next().flatten(),
        )
        .fixture();
    assert_eq!(report.stats.len(), 2);
    assert_eq!(report.stats[0].pass, "unchanged");
    assert_eq!(report.stats[1].pass, "unchanged-module");
    assert_eq!(events[0].allocated_bytes, Some(44));
    assert!(events[0].to_string().contains("allocated_bytes=44"));
    assert_eq!(events[1].allocated_bytes, Some(8));

    let mut manager = PassManager::with_options(PassManagerOptions {
        max_iterations: 1,
        verify_after_each_pass: true,
    });
    manager.add_function_pass(UnchangedPass);
    let mut snapshots = [Some(9_u128), Some(8_u128)].into_iter();
    let mut events = Vec::new();
    manager
        .run_with_observer_and_allocation_snapshot(
            &mut Module {
                functions: vec![leaf()],
            },
            |event| events.push(event.clone()),
            || snapshots.next().flatten(),
        )
        .fixture();
    assert_eq!(events[0].allocated_bytes, None);
    assert!(!events[0].to_string().contains("allocated_bytes"));
}

#[test]
fn allocation_snapshot_omits_delta_when_the_initial_snapshot_is_unavailable() {
    let mut manager = PassManager::with_options(PassManagerOptions {
        max_iterations: 1,
        verify_after_each_pass: true,
    });
    manager.add_function_pass(UnchangedPass);
    let mut snapshots = [None, Some(61_u128)].into_iter();
    let mut events = Vec::new();
    let report = manager
        .run_with_observer_and_allocation_snapshot(
            &mut Module {
                functions: vec![leaf()],
            },
            |event| events.push(event.clone()),
            || snapshots.next().flatten(),
        )
        .fixture();

    assert_eq!(report.stats.len(), 1);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].allocated_bytes, None);
    assert!(!events[0].to_string().contains("allocated_bytes"));
}

#[test]
fn allocation_snapshot_omits_delta_when_the_final_snapshot_is_unavailable() {
    let mut manager = PassManager::with_options(PassManagerOptions {
        max_iterations: 1,
        verify_after_each_pass: true,
    });
    manager.add_function_pass(UnchangedPass);
    let mut snapshots = [Some(61_u128), None].into_iter();
    let mut events = Vec::new();
    let report = manager
        .run_with_observer_and_allocation_snapshot(
            &mut Module {
                functions: vec![leaf()],
            },
            |event| events.push(event.clone()),
            || snapshots.next().flatten(),
        )
        .fixture();

    assert_eq!(report.stats.len(), 1);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].allocated_bytes, None);
    assert!(!events[0].to_string().contains("allocated_bytes"));
}

use super::tests_support::{Fixture, caller, leaf};
use crate::{
    FunctionPass, InlineDirectCalls, Module, ModulePass, PassError, PassManager,
    PassManagerOptions, PassResult,
};
use ncl_ir::{Constant, ConstantIndex, Function, Op, OpKind, Terminator, Ty, ValueId};

#[derive(Debug)]
struct TogglePass {
    changed: bool,
}

impl FunctionPass for TogglePass {
    fn name(&self) -> &'static str {
        "toggle"
    }
    fn run(&mut self, _function: &mut Function, _module: &Module) -> PassResult {
        let changed = self.changed;
        self.changed = false;
        Ok(changed)
    }
}

#[derive(Debug)]
struct ModuleToggle;
impl ModulePass for ModuleToggle {
    fn name(&self) -> &'static str {
        "module-toggle"
    }
    fn run(&mut self, _module: &mut Module) -> PassResult {
        Ok(false)
    }
}

#[derive(Debug)]
struct FailingPass;
impl FunctionPass for FailingPass {
    fn name(&self) -> &'static str {
        "failing"
    }
    fn run(&mut self, _function: &mut Function, _module: &Module) -> PassResult {
        Err(PassError::new("inner", "boom"))
    }
}

#[derive(Debug)]
struct InvalidatingPass;
impl FunctionPass for InvalidatingPass {
    fn name(&self) -> &'static str {
        "invalidating"
    }
    fn run(&mut self, function: &mut Function, _module: &Module) -> PassResult {
        function.blocks[0].ops.push(Op {
            results: vec![(ValueId(0), Ty::Word)],
            kind: OpKind::Move { value: ValueId(0) },
            loc: None,
        });
        Ok(true)
    }
}

#[test]
fn manager_reports_module_passes_limits_and_errors() {
    let options = PassManagerOptions {
        max_iterations: 1,
        verify_after_each_pass: true,
    };
    let mut manager = PassManager::with_options(options);
    manager.add_function_pass(TogglePass { changed: true });
    manager.add_module_pass(ModuleToggle);
    let report = manager
        .run(&mut Module {
            functions: vec![leaf()],
        })
        .fixture();
    assert!(report.hit_iteration_limit);
    assert_eq!(report.stats.len(), 2);
    let options = PassManagerOptions::default().without_verification();
    let mut manager = PassManager::with_options(options);
    manager.add_function_pass(FailingPass);
    let Err(error) = manager.run(&mut Module {
        functions: vec![leaf()],
    }) else {
        std::process::exit(1)
    };
    assert_eq!(error.pass, "failing");
    assert!(error.to_string().contains("failing"));
}

#[test]
fn manager_rejects_an_invalid_function_after_a_pass() {
    let mut manager = PassManager::new();
    manager.add_function_pass(InvalidatingPass);
    let Err(error) = manager.run(&mut Module {
        functions: vec![leaf()],
    }) else {
        std::process::exit(1)
    };
    assert_eq!(error.pass, "invalidating");
    assert!(error.message.contains("DuplicateValue"));
}

#[test]
fn default_pipeline_registers_phase_three_passes_in_order() {
    let mut manager = PassManager::new();
    manager.add_default_optimization_pipeline();
    let report = manager
        .run(&mut Module {
            functions: vec![leaf()],
        })
        .fixture();
    let names = report
        .stats
        .iter()
        .map(|stat| stat.pass.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec!["inline-direct-calls", "global-value-numbering", "sccp"]
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn conservative_guards_and_manager_edges_are_exercised() {
    let mut invalid = leaf();
    invalid.blocks[0].ops.push(Op {
        results: vec![(ValueId(0), Ty::Word)],
        kind: OpKind::Move { value: ValueId(0) },
        loc: None,
    });
    assert!(
        Module {
            functions: vec![invalid]
        }
        .verify()
        .is_err()
    );
    let mut zero = PassManager::with_options(PassManagerOptions {
        max_iterations: 0,
        verify_after_each_pass: true,
    });
    let report = zero
        .run(&mut Module {
            functions: vec![leaf()],
        })
        .fixture();
    assert_eq!(report.iterations, 0);
    assert!(!report.hit_iteration_limit);
    let mut guarded = leaf();
    guarded.blocks[0].ops.push(Op {
        results: vec![],
        kind: OpKind::MakeClosure {
            entry: ValueId(0),
            captures: vec![],
        },
        loc: None,
    });
    let mut pass = InlineDirectCalls::default();
    assert!(!pass.run(&mut guarded, &Module::default()).fixture());
    let mut caller_function = caller();
    caller_function.blocks[0].ops[0].kind = OpKind::Move { value: ValueId(0) };
    assert!(
        !pass
            .run(
                &mut caller_function,
                &Module {
                    functions: vec![leaf()]
                }
            )
            .fixture()
    );
    caller_function.blocks[0].ops[0].kind = OpKind::Const {
        result: ConstantIndex(0),
    };
    caller_function.constants[0] = Constant::Nil;
    assert!(
        !pass
            .run(
                &mut caller_function,
                &Module {
                    functions: vec![leaf()]
                }
            )
            .fixture()
    );
    let mut non_return = leaf();
    non_return.blocks[0].terminator = Terminator::Unreachable;
    assert!(
        !pass
            .run(
                &mut caller(),
                &Module {
                    functions: vec![non_return]
                }
            )
            .fixture()
    );
    let mut bad_arity = caller();
    if let OpKind::Call { args, .. } = &mut bad_arity.blocks[0].ops[1].kind {
        args.clear();
    }
    assert!(
        !pass
            .run(
                &mut bad_arity,
                &Module {
                    functions: vec![leaf()]
                }
            )
            .fixture()
    );
    let mut bad_return = leaf();
    bad_return.blocks[0].terminator = Terminator::Return {
        values: vec![ValueId(99)],
    };
    assert!(
        !pass
            .run(
                &mut caller(),
                &Module {
                    functions: vec![bad_return]
                }
            )
            .fixture()
    );
}

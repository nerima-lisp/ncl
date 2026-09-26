#![allow(clippy::cast_possible_truncation)]

use crate::remap::{next_value, remap_kind, remap_op_values, remap_term_values};
use crate::{
    FunctionPass, InlineDirectCalls, Module, ModulePass, PassError, PassManager,
    PassManagerOptions, PassResult,
};
use ncl_ir::{Constant, ConstantIndex, Function, Op, OpKind, Terminator, ValueId};
use ncl_ir::{FunctionBuilder, FunctionId, Param, Ty};
use std::collections::HashMap;
use std::fmt::Debug;

trait Fixture<T> {
    fn fixture(self) -> T;
}

impl<T, E: Debug> Fixture<T> for Result<T, E> {
    fn fixture(self) -> T {
        self.unwrap_or_else(|_| std::process::exit(1))
    }
}

fn leaf() -> Function {
    let mut b = FunctionBuilder::new(
        FunctionId(2),
        "leaf",
        vec![Param {
            name: "x".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let x = b
        .push_op(OpKind::Move { value: ValueId(0) }, &[Ty::Word])
        .fixture()[0];
    b.terminate(Terminator::Return { values: vec![x] })
        .fixture();
    b.finish()
}

fn caller() -> Function {
    let mut b = FunctionBuilder::new(
        FunctionId(1),
        "caller",
        vec![Param {
            name: "x".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let entry = b.add_constant(Constant::FunctionEntry(FunctionId(2)));
    let f = b
        .push_op(OpKind::Const { result: entry }, &[Ty::Word])
        .fixture()[0];
    let call = b
        .push_op(
            OpKind::Call {
                function: f,
                args: vec![ValueId(0)],
            },
            &[Ty::Word],
        )
        .fixture()[0];
    b.terminate(Terminator::Return { values: vec![call] })
        .fixture();
    b.finish()
}

#[test]
fn direct_leaf_inlines_and_text_round_trips() {
    let mut module = Module {
        functions: vec![caller(), leaf()],
    };
    let text = module.functions[0].to_string();
    assert_eq!(ncl_ir::parse(&text).fixture().to_string(), text);
    let mut manager = PassManager::new();
    manager.add_function_pass(InlineDirectCalls::default());
    let report = manager.run(&mut module).fixture();
    assert!(report.stats.iter().any(|stat| stat.changed));
    assert!(
        module.functions[0].blocks[0]
            .ops
            .iter()
            .all(|op| !matches!(op.kind, OpKind::Call { .. }))
    );
    module.verify().fixture();
}

#[test]
fn recursive_and_unsafe_callees_are_skipped() {
    let mut recursive = leaf();
    recursive.id = FunctionId(2);
    let entry = ConstantIndex(recursive.constants.len() as u32);
    recursive
        .constants
        .push(Constant::FunctionEntry(FunctionId(2)));
    recursive.blocks[0].ops.insert(
        0,
        Op {
            results: vec![(ValueId(2), Ty::Word)],
            kind: OpKind::Const { result: entry },
            loc: None,
        },
    );
    recursive.blocks[0].ops.push(Op {
        results: vec![],
        kind: OpKind::Safepoint,
        loc: None,
    });
    recursive.blocks[0].ops.push(Op {
        results: vec![(ValueId(3), Ty::Word)],
        kind: OpKind::Call {
            function: ValueId(2),
            args: vec![ValueId(0)],
        },
        loc: None,
    });
    recursive.blocks[0].terminator = Terminator::Return {
        values: vec![ValueId(3)],
    };
    let mut module = Module {
        functions: vec![caller(), recursive],
    };
    module.functions[0].blocks[0].ops.insert(
        1,
        Op {
            results: vec![],
            kind: OpKind::Safepoint,
            loc: None,
        },
    );
    let before = module.functions[0].clone();
    let mut manager = PassManager::new();
    manager.add_function_pass(InlineDirectCalls::default());
    manager.run(&mut module).fixture();
    assert_eq!(module.functions[0], before);
}

#[test]
fn threshold_skips() {
    let mut module = Module {
        functions: vec![caller(), leaf()],
    };
    module.functions[0].blocks[0].ops.insert(
        1,
        Op {
            results: vec![],
            kind: OpKind::Safepoint,
            loc: None,
        },
    );
    let mut manager = PassManager::new();
    manager.add_function_pass(InlineDirectCalls { max_ops: 0 });
    manager.run(&mut module).fixture();
    assert!(
        module.functions[0].blocks[0]
            .ops
            .iter()
            .any(|op| matches!(op.kind, OpKind::Call { .. }))
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn remap_helpers_cover_ir_shapes() {
    let values = HashMap::from([(ValueId(0), ValueId(9))]);
    let mut constants = HashMap::new();
    let mut caller_constants = Vec::new();
    let mut callee = leaf();
    callee.constants.push(Constant::Nil);
    let kinds = vec![
        OpKind::Const {
            result: ConstantIndex(0),
        },
        OpKind::Move { value: ValueId(0) },
        OpKind::Load {
            address: ValueId(0),
        },
        OpKind::Store {
            address: ValueId(0),
            value: ValueId(0),
        },
        OpKind::LoadField {
            object: ValueId(0),
            field: 1,
        },
        OpKind::StoreField {
            object: ValueId(0),
            field: 1,
            value: ValueId(0),
        },
        OpKind::Alloc { words: 1 },
        OpKind::LoadArg { index: 0 },
        OpKind::Builtin {
            name: "x".into(),
            args: vec![ValueId(0)],
        },
        OpKind::Prim {
            op: ncl_ir::Prim::Car,
            args: vec![ValueId(0)],
            condition: None,
        },
        OpKind::Compare {
            op: ncl_ir::Compare::Eq,
            left: ValueId(0),
            right: ValueId(0),
        },
        OpKind::Convert {
            op: ncl_ir::Convert::I64ToWord,
            value: ValueId(0),
        },
        OpKind::Call {
            function: ValueId(0),
            args: vec![ValueId(0)],
        },
        OpKind::CallIndirect {
            callee: ValueId(0),
            args: vec![ValueId(0)],
        },
        OpKind::MakeClosure {
            entry: ValueId(0),
            captures: vec![ValueId(0)],
        },
        OpKind::CallClosure {
            closure: ValueId(0),
            args: vec![ValueId(0)],
        },
        OpKind::SetMultipleValues {
            values: vec![ValueId(0)],
        },
        OpKind::Safepoint,
        OpKind::EnterHandler {
            region: ncl_ir::HandlerRegionId(0),
        },
        OpKind::LeaveHandler {
            region: ncl_ir::HandlerRegionId(0),
        },
    ];
    for kind in kinds {
        let _ = remap_kind(
            &kind,
            &values,
            &mut constants,
            &mut caller_constants,
            &callee,
        );
        remap_op_values(
            &mut Op {
                results: vec![],
                kind,
                loc: None,
            },
            &values,
        );
    }
    let mut terms = vec![
        Terminator::Jump {
            target: ncl_ir::BlockId(1),
            args: vec![ValueId(0)],
        },
        Terminator::Branch {
            condition: ValueId(0),
            then_target: ncl_ir::BlockId(1),
            then_args: vec![ValueId(0)],
            else_target: ncl_ir::BlockId(2),
            else_args: vec![ValueId(0)],
        },
        Terminator::Switch {
            value: ValueId(0),
            cases: vec![(1, ncl_ir::BlockId(1), vec![ValueId(0)])],
            default: ncl_ir::BlockId(2),
            default_args: vec![ValueId(0)],
        },
        Terminator::CallReturn {
            function: ValueId(0),
            args: vec![ValueId(0)],
        },
        Terminator::TailCall {
            function: ValueId(0),
            args: vec![ValueId(0)],
        },
        Terminator::Return {
            values: vec![ValueId(0)],
        },
        Terminator::Throw {
            condition: ValueId(0),
        },
        Terminator::Unreachable,
    ];
    for term in &mut terms {
        remap_term_values(term, &values);
    }
    assert_eq!(next_value(&callee), 2);
}

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
        std::process::exit(1);
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
        vec!["inline-direct-calls", "global-value-numbering", "sccp",]
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

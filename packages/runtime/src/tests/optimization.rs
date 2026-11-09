use super::{OptimizationMode, RuntimeOptions, verify_and_optimize};
use crate::{Runtime, RuntimeError};
use ncl_ir::{FunctionBuilder, FunctionId, OpKind, Param, Terminator, Ty, ValueId};
use std::cell::Cell;
use std::rc::Rc;

fn verified_module_with_unused_move() -> ncl_opt::Module {
    let mut builder = FunctionBuilder::new(
        FunctionId(7),
        "runtime-options",
        vec![Param {
            name: "value".to_owned(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    builder
        .push_op(OpKind::Move { value: ValueId(0) }, &[Ty::Word])
        .unwrap_or_else(|error| panic!("runtime-options operation failed: {error}"));
    builder
        .terminate(Terminator::Return {
            values: vec![ValueId(0)],
        })
        .unwrap_or_else(|error| panic!("runtime-options terminator failed: {error}"));
    ncl_opt::Module {
        functions: vec![builder.finish()],
    }
}

#[test]
fn optimization_off_preserves_unused_move_removed_by_default_pipeline() {
    let mut unoptimized = verified_module_with_unused_move();
    let original = unoptimized.clone();
    let options = RuntimeOptions::new(OptimizationMode::Off, true);
    let mut pass_runner = crate::optimization::default_pass_runner(options);

    let result = verify_and_optimize(&mut unoptimized, options, &mut pass_runner);

    assert!(
        result.is_ok(),
        "verified IR should remain compilable: {result:?}"
    );
    assert_eq!(unoptimized, original);

    let mut optimized = original;
    let options = RuntimeOptions::default();
    let mut pass_runner = crate::optimization::default_pass_runner(options);
    let result = verify_and_optimize(&mut optimized, options, &mut pass_runner);

    assert!(result.is_ok(), "default optimization failed: {result:?}");
    assert!(optimized.functions[0].blocks[0].ops.is_empty());
    assert_eq!(
        optimized.functions[0].blocks[0].terminator,
        Terminator::Return {
            values: vec![ValueId(0)]
        }
    );
}

#[test]
fn pass_diagnostics_preserve_optimized_ir() {
    let mut module = verified_module_with_unused_move();
    let options = RuntimeOptions::new(OptimizationMode::Default, true);

    assert!(options.opt_diagnostics);

    let mut pass_runner = crate::optimization::default_pass_runner(options);
    let result = verify_and_optimize(&mut module, options, &mut pass_runner);

    assert!(
        result.is_ok(),
        "optimization with diagnostics failed: {result:?}"
    );
    assert!(module.functions[0].blocks[0].ops.is_empty());
    assert_eq!(
        module.functions[0].blocks[0].terminator,
        Terminator::Return {
            values: vec![ValueId(0)]
        }
    );
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "the test needs a runtime to observe the caller-provided pass runner"
)]
fn caller_pass_runner_preserves_eval_results_and_observes_pass_events() {
    let pass_events = Rc::new(Cell::new(0));
    let observed_events = Rc::clone(&pass_events);
    let mut runtime =
        Runtime::with_options_and_pass_runner(RuntimeOptions::default(), move |passes, module| {
            passes.run_with_observer(module, |_| {
                observed_events.set(observed_events.get() + 1);
            })
        })
        .expect("runtime initializes with the caller-provided pass runner");
    let value = runtime
        .eval("(+ 20 22)")
        .expect("the pass runner compiles and evaluates the source");

    assert_eq!(runtime.format_result(value), "42");
    assert!(pass_events.get() > 0);
    assert!(format!("{runtime:?}").contains("StoredPassRunner"));
}

#[test]
fn invalid_ir_is_rejected_before_optimization_without_mutating_module() {
    let mut module = verified_module_with_unused_move();
    module.functions[0].blocks[0].terminator = Terminator::Return {
        values: vec![ValueId(99)],
    };
    let original = module.clone();

    let options = RuntimeOptions::new(OptimizationMode::Off, false);
    let mut pass_runner = crate::optimization::default_pass_runner(options);
    let result = verify_and_optimize(&mut module, options, &mut pass_runner);

    assert_eq!(module, original);
    assert!(matches!(
        result,
        Err(RuntimeError::Native(message))
            if message.starts_with("IR verification before optimization failed: ")
                && message.contains("UndefinedValue(ValueId(99))")
    ));
}

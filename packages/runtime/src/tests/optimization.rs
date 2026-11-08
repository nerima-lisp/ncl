use super::{OptimizationMode, RuntimeOptions, verify_and_optimize};
use crate::RuntimeError;
use ncl_ir::{FunctionBuilder, FunctionId, OpKind, Param, Terminator, Ty, ValueId};

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

    let result = verify_and_optimize(
        &mut unoptimized,
        RuntimeOptions::new(OptimizationMode::Off, true),
    );

    assert!(
        result.is_ok(),
        "verified IR should remain compilable: {result:?}"
    );
    assert_eq!(unoptimized, original);

    let mut optimized = original;
    let result = verify_and_optimize(&mut optimized, RuntimeOptions::default());

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
fn pass_diagnostics_without_allocation_tracking_preserve_optimized_ir() {
    let mut module = verified_module_with_unused_move();
    let options = RuntimeOptions::new(OptimizationMode::Default, true)
        .with_pass_allocation_diagnostics(false);

    assert!(options.opt_diagnostics);
    assert!(!options.pass_allocation_diagnostics);

    let result = verify_and_optimize(&mut module, options);

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
fn invalid_ir_is_rejected_before_optimization_without_mutating_module() {
    let mut module = verified_module_with_unused_move();
    module.functions[0].blocks[0].terminator = Terminator::Return {
        values: vec![ValueId(99)],
    };
    let original = module.clone();

    let result = verify_and_optimize(
        &mut module,
        RuntimeOptions::new(OptimizationMode::Off, false),
    );

    assert_eq!(module, original);
    assert!(matches!(
        result,
        Err(RuntimeError::Native(message))
            if message.starts_with("IR verification before optimization failed: ")
                && message.contains("UndefinedValue(ValueId(99))")
    ));
}

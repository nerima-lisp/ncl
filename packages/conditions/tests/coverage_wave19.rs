#![allow(
    clippy::unwrap_used,
    missing_docs,
    reason = "coverage tests assert on condition behavior"
)]

//! Nineteenth-wave coverage for inherited condition reports and conversion.

use ncl_conditions::{
    condition_class, condition_from_lisp_error, condition_report, make_condition,
    ConditionIdentifier,
};
use ncl_object::{
    make_cons, make_string, CellError, FunctionObject, LispError, Package, ProgramError, Runtime,
    ThreadContext, Word,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_conditions::register(&runtime).unwrap();
    (runtime, ctx)
}

fn builtin(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    package: &str,
    name: &str,
) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, package, name).unwrap()).unwrap()
}

fn symbol(ctx: &mut ThreadContext, runtime: &Runtime, package: &str, name: &str) -> Word {
    let package = runtime.ensure_package(ctx, package).unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

#[test]
fn custom_report_wins_over_the_inherited_simple_condition_report() {
    let (runtime, mut ctx) = setup();
    let define = builtin(&runtime, &mut ctx, "NCL-EXT", "DEFINE-CONDITION-CLASS");
    let name = symbol(&mut ctx, &runtime, "NCL-C5", "REPORTED-SIMPLE-CONDITION");
    let simple = symbol(&mut ctx, &runtime, "COMMON-LISP", "SIMPLE-CONDITION");
    let parents = make_cons(&mut ctx, &runtime, simple, Word::NIL).unwrap();
    let report = make_string(
        &mut ctx,
        &runtime,
        &"custom report".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, define, &[name, parents, Word::NIL, report]),
        Ok(name)
    );

    let class = condition_class(&mut ctx, &runtime, "REPORTED-SIMPLE-CONDITION").unwrap();
    let control = make_string(
        &mut ctx,
        &runtime,
        &"inherited ~a".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    let instance = make_condition(&mut ctx, &runtime, class, &[control, Word::NIL]).unwrap();
    assert_eq!(
        condition_report(&ctx, instance).as_deref(),
        Some("custom report")
    );
}

#[test]
fn conversion_preserves_specific_cell_and_keyword_condition_kinds() {
    let (runtime, mut ctx) = setup();
    let name = symbol(&mut ctx, &runtime, "NCL-C5", "MISSING-VARIABLE");
    let unbound = condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::CellError(CellError::UnboundVariable { name }),
    )
    .unwrap();
    assert_eq!(
        ncl_conditions::condition_class_of(&ctx, unbound).unwrap(),
        condition_class(&mut ctx, &runtime, "UNBOUND-VARIABLE").unwrap()
    );

    let keyword = condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::ProgramError(ProgramError::UnknownKeyword),
    )
    .unwrap();
    assert_eq!(
        ncl_conditions::condition_class_of(&ctx, keyword).unwrap(),
        ConditionIdentifier::ProgramError
            .class(&mut ctx, &runtime)
            .unwrap()
    );
}

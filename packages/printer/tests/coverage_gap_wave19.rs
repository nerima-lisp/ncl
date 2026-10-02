#![allow(
    clippy::unwrap_used,
    missing_docs,
    reason = "coverage tests assert on printer output"
)]

//! Nineteenth-wave coverage for type-error report printing.

use ncl_object::{Package, Runtime, ThreadContext, Word, make_string};
use ncl_printer::{PrintOptions, StringSink, write};

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_conditions::register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    (runtime, ctx)
}

fn symbol(ctx: &mut ThreadContext, runtime: &Runtime, package: &str, name: &str) -> Word {
    let package = runtime.ensure_package(ctx, package).unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn render(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    condition: Word,
    options: PrintOptions,
) -> String {
    let mut sink = StringSink::new();
    write(ctx, runtime, condition, &mut sink, &options).unwrap();
    sink.into_string()
}

#[test]
fn princ_uses_type_error_report_while_prin1_keeps_instance_syntax() {
    let (runtime, mut ctx) = context();
    let class = ncl_conditions::condition_class(&mut ctx, &runtime, "TYPE-ERROR").unwrap();
    let datum = make_string(&mut ctx, &runtime, &"wrong".chars().collect::<Vec<_>>()).unwrap();
    let expected = symbol(&mut ctx, &runtime, "COMMON-LISP", "FIXNUM");
    let condition =
        ncl_conditions::make_condition(&mut ctx, &runtime, class, &[datum, expected]).unwrap();

    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            condition,
            PrintOptions::new().with_escape(false)
        ),
        "The value wrong is not of type FIXNUM."
    );
    assert!(render(&runtime, &mut ctx, condition, PrintOptions::new()).starts_with("#<INSTANCE "));
}

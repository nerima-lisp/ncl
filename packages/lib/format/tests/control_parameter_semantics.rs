#![allow(missing_docs, clippy::expect_used, clippy::unwrap_used)]

use ncl_lib_format::{FormatErrorKind, execute, parse};
use ncl_object::{Runtime, ThreadContext, make_bignum_from_i128};
use ncl_printer::StringSink;

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register context");
    (runtime, ctx)
}

#[test] // CLHS 22.3.9.2
fn rejects_character_parameter_for_up_arrow() {
    let (runtime, mut ctx) = context();
    let control = parse("~'x^").expect("control");
    let mut sink = StringSink::new();

    let error = execute(&control, &[], &mut ctx, &runtime, &mut sink)
        .expect_err("up-arrow parameters must be integer-valued");

    assert_eq!(error.position, 0);
    assert_eq!(error.directive, Some('^'));
    assert_eq!(error.kind, FormatErrorKind::InvalidParameter);
}

#[test] // CLHS 22.3.7.2
fn selects_default_for_bignum_indices_outside_signed_machine_range() {
    let (runtime, mut ctx) = context();
    let control = parse("~[zero~;one~:;other~]").expect("control");

    for index in [i128::from(i64::MAX) + 1, i128::from(i64::MIN) - 1] {
        let selector = make_bignum_from_i128(&mut ctx, &runtime, index)
            .map(Into::into)
            .expect("bignum selector");
        let mut sink = StringSink::new();

        assert_eq!(
            execute(&control, &[selector], &mut ctx, &runtime, &mut sink),
            Ok(1),
            "selector {index}"
        );
        assert_eq!(sink.into_string(), "other", "selector {index}");
    }
}

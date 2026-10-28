use super::{FloatFormat, current_read_options};
use crate::{Runtime, RuntimeError};
use ncl_object::ObjectError;

#[test]
fn current_read_options_maps_dynamic_float_format_aliases() {
    for (format, binding, expected) in [
        (
            "SHORT-FLOAT",
            "(setq *read-default-float-format* 'short-float)",
            FloatFormat::SingleFloat,
        ),
        (
            "SINGLE-FLOAT",
            "(setq *read-default-float-format* 'single-float)",
            FloatFormat::SingleFloat,
        ),
        (
            "DOUBLE-FLOAT",
            "(setq *read-default-float-format* 'double-float)",
            FloatFormat::DoubleFloat,
        ),
        (
            "LONG-FLOAT",
            "(setq *read-default-float-format* 'long-float)",
            FloatFormat::DoubleFloat,
        ),
    ] {
        let mut runtime =
            Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
        runtime.eval(binding).unwrap_or_else(|error| {
            panic!("setting *READ-DEFAULT-FLOAT-FORMAT* {format} failed: {error}")
        });

        let options = current_read_options(&mut runtime, None).unwrap_or_else(|error| {
            panic!("reading *READ-DEFAULT-FLOAT-FORMAT* {format} failed: {error}")
        });
        assert_eq!(options.default_float_format(), expected, "{format}");
    }
}

#[test]
fn current_read_options_rejects_invalid_dynamic_values() {
    for (name, binding) in [
        ("*PACKAGE*", "(setq *package* nil)"),
        ("*READTABLE*", "(setq *readtable* nil)"),
        ("*READ-BASE* type", "(setq *read-base* nil)"),
        ("*READ-BASE* negative", "(setq *read-base* -1)"),
        ("*READ-BASE* range", "(setq *read-base* 1)"),
        (
            "*READ-DEFAULT-FLOAT-FORMAT* type",
            "(setq *read-default-float-format* 42)",
        ),
        (
            "*READ-DEFAULT-FLOAT-FORMAT* unsupported symbol",
            "(setq *read-default-float-format* 'rational)",
        ),
    ] {
        let mut runtime =
            Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
        if let Err(error) = runtime.eval(binding) {
            assert!(
                matches!(error, RuntimeError::Object(ObjectError::TypeError)),
                "setting {name} returned the wrong error: {error:?}"
            );
        }

        let Err(error) = current_read_options(&mut runtime, None) else {
            panic!("invalid dynamic reader value was accepted");
        };
        assert!(
            matches!(error, RuntimeError::Object(ObjectError::TypeError)),
            "{name} returned the wrong error: {error:?}"
        );
    }
}

#[test]
fn current_read_options_rejects_a_deleted_dynamic_package() {
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    let options = current_read_options(&mut runtime, None)
        .unwrap_or_else(|error| panic!("initial dynamic reader options should be valid: {error}"));
    runtime
        .eval(
            "(setq *package* (make-package \"NCL-READ-OPTIONS-DELETED\" \
             :use (list (find-package \"COMMON-LISP\"))))",
        )
        .unwrap_or_else(|error| panic!("dynamic package setup should succeed: {error}"));
    runtime
        .eval("(delete-package *package*)")
        .unwrap_or_else(|error| panic!("deleting the current package should succeed: {error}"));

    let Err(error) = current_read_options(&mut runtime, Some(options)) else {
        panic!("a deleted dynamic package should not supply a reader package name");
    };
    assert!(
        matches!(
            error,
            RuntimeError::Front(ncl_compiler_front::FrontError::Object(
                ObjectError::TypeError
            ))
        ),
        "deleted *PACKAGE* returned the wrong error: {error:?}"
    );
}

#[test]
fn current_read_options_rejects_a_deleted_common_lisp_package() {
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    let options = current_read_options(&mut runtime, None)
        .unwrap_or_else(|error| panic!("initial dynamic reader options should be valid: {error}"));

    match runtime.eval("(delete-package (find-package \"COMMON-LISP\"))") {
        Err(RuntimeError::Object(ObjectError::Layout)) => {}
        Err(error) => panic!("deleting COMMON-LISP returned the wrong error: {error:?}"),
        Ok(value) => panic!(
            "deleting COMMON-LISP unexpectedly completed reader setup: {}",
            runtime.format_result(value)
        ),
    }

    let Err(error) = current_read_options(&mut runtime, Some(options)) else {
        panic!("a missing COMMON-LISP package should prevent dynamic reader lookup");
    };
    assert!(
        matches!(error, RuntimeError::Object(ObjectError::Layout)),
        "missing COMMON-LISP returned the wrong error: {error:?}"
    );
}

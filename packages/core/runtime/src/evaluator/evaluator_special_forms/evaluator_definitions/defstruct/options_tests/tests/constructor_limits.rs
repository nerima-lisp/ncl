use super::*;

#[test]
fn defstruct_bare_constructor_option_uses_the_default_name() {
    let values = Runtime::new()
        .eval_source(
            "(defstruct (bare-constructor-struct (:constructor)) field)
             (funcall #'bare-constructor-struct-field
                      (make-bare-constructor-struct :field 9))",
        )
        .unwrap_or_else(|error| {
            panic!("a bare :constructor option should keep the default name: {error}")
        });
    assert_eq!(
        values
            .last()
            .unwrap_or_else(|| panic!("expected a value"))
            .to_string(),
        "9"
    );
}

#[test]
fn defstruct_rejects_too_many_constructor_arguments() {
    let error = eval_err(
        "(defstruct (too-many-constructor-args-struct
                      (:constructor make-too-many-constructor-args-struct (field) extra))
            field)",
    );
    assert!(matches!(
        error,
        RuntimeError::InvalidForm { message, .. }
            if message == "defstruct :constructor accepts at most a name and a lambda list"
    ));
}

#[test]
fn defstruct_rejects_a_malformed_constructor_name() {
    let error = eval_err("(defstruct (bad-constructor-name-struct (:constructor (nested))) field)");
    assert!(matches!(
        error,
        RuntimeError::InvalidForm { message, .. }
            if message == "defstruct :constructor must name a symbol or NIL"
    ));
}

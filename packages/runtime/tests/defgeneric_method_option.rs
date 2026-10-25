#![allow(missing_docs)]

use ncl_runtime::Runtime;
use ncl_sys::Word;

#[test]
fn defgeneric_method_option_registers_a_method_metaobject_and_dispatches() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let result = runtime
        .eval(
            "(progn \
               (defgeneric ncl-defgeneric-method-option-test (value) \
                 (:method ((value integer)) value)) \
               (and \
                 (typep (find-method #'ncl-defgeneric-method-option-test nil \
                                     (list (find-class 'integer)) nil) \
                        'method) \
                 (= (ncl-defgeneric-method-option-test 17) 17)))",
        )
        .unwrap_or_else(|error| panic!("DEFGENERIC :METHOD evaluation failed: {error}"));
    assert_eq!(result, Word::TRUE);
}

#[test]
fn defgeneric_redefinition_preserves_the_captured_generic_function() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let result = runtime
        .eval(
            "(progn \
               (defgeneric ncl-defgeneric-redefinition-test (value)) \
               (defparameter *ncl-defgeneric-captured-function* \
                 #'ncl-defgeneric-redefinition-test) \
               (defgeneric ncl-defgeneric-redefinition-test (value) \
                 (:method ((value integer)) value)) \
               (and (eq *ncl-defgeneric-captured-function* \
                        #'ncl-defgeneric-redefinition-test) \
                    (= (ncl-defgeneric-redefinition-test 23) 23)))",
        )
        .unwrap_or_else(|error| panic!("DEFGENERIC redefinition failed: {error}"));
    assert_eq!(result, Word::TRUE);
}

#[test]
fn defgeneric_rejects_function_and_macro_name_collisions() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let function_collision = runtime
        .eval(
            "(progn \
               (defun ncl-defgeneric-function-collision-test (value) value) \
               (handler-case \
                 (progn (defgeneric ncl-defgeneric-function-collision-test (value)) nil) \
                 (program-error () (= (ncl-defgeneric-function-collision-test 31) 31))))",
        )
        .unwrap_or_else(|error| panic!("function collision handler failed: {error}"));
    assert_eq!(function_collision, Word::TRUE);

    runtime
        .eval("(defmacro ncl-defgeneric-macro-collision-test (value) value)")
        .unwrap_or_else(|error| panic!("DEFMACRO setup failed: {error}"));
    let macro_collision = runtime
        .eval(
            "(and \
               (handler-case \
                 (progn (defgeneric ncl-defgeneric-macro-collision-test (value)) nil) \
                 (program-error () t)) \
               (not (null (macro-function 'ncl-defgeneric-macro-collision-test))))",
        )
        .unwrap_or_else(|error| panic!("macro collision handler failed: {error}"));
    assert_eq!(macro_collision, Word::TRUE);
}

#[test]
fn defgeneric_rejects_unknown_and_non_keyword_options() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    for (form, check) in [
        (
            "(defgeneric ncl-defgeneric-unknown-option-test (value) \
               (:ncl-unknown-option))",
            "unknown option",
        ),
        (
            "(defgeneric ncl-defgeneric-symbol-option-test (value) \
               (method ((value integer)) value))",
            "non-keyword method option",
        ),
        (
            "(defgeneric ncl-defgeneric-malformed-method-test (value) (:method))",
            "missing inline method lambda list",
        ),
    ] {
        let result = runtime
            .eval(&format!(
                "(handler-case (progn {form} nil) (program-error () t))"
            ))
            .unwrap_or_else(|error| panic!("{check} handler failed: {error}"));
        assert_eq!(result, Word::TRUE, "{check}");
    }
}

#[test]
fn defgeneric_accepts_standard_options_and_preserves_standalone_methods() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    runtime
        .eval(
            "(defgeneric ncl-defgeneric-method-provenance-test (value) \
               (:argument-precedence-order (value)) \
               (:declare (special value)) \
               (:documentation \"test generic\") \
               (:generic-function-class standard-generic-function) \
               (:method-class standard-method) \
               (:method-combination standard) \
               (:method ((value integer)) :old-inline))",
        )
        .unwrap_or_else(|error| panic!("initial DEFGENERIC failed: {error}"));
    runtime
        .eval(
            "(defmethod ncl-defgeneric-method-provenance-test ((value (eql 99))) \
               :standalone)",
        )
        .unwrap_or_else(|error| panic!("standalone DEFMETHOD failed: {error}"));
    runtime
        .eval(
            "(defgeneric ncl-defgeneric-method-provenance-test (value) \
               (:method ((value (eql 100))) :new-inline))",
        )
        .unwrap_or_else(|error| panic!("redefining DEFGENERIC failed: {error}"));
    let result = runtime
        .eval(
            "(and \
               (eq (ncl-defgeneric-method-provenance-test 99) :standalone) \
               (eq (ncl-defgeneric-method-provenance-test 100) :new-inline) \
               (eq (find-method #'ncl-defgeneric-method-provenance-test nil \
                                (list (find-class 'integer)) nil) nil))",
        )
        .unwrap_or_else(|error| panic!("method dispatch after redefinition failed: {error}"));
    assert_eq!(result, Word::TRUE);
}

#[test]
fn defmethod_implicitly_creates_generic_with_method_lambda_list() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    runtime
        .eval("(deftype ncl-defmethod-implicit-property-test () integer)")
        .unwrap_or_else(|error| panic!("DEFTYPE setup failed: {error}"));
    runtime
        .eval("(define-condition ncl-defmethod-implicit-test-condition (condition) ())")
        .unwrap_or_else(|error| panic!("condition setup failed: {error}"));
    runtime
        .eval(
            "(defmethod ncl-defmethod-implicit-property-test \
               ((value ncl-defmethod-implicit-test-condition) &optional fallback) \
               (if fallback fallback 47))",
        )
        .unwrap_or_else(|error| panic!("implicit DEFMETHOD generic creation failed: {error}"));
    runtime
        .eval(
            "(defmethod ncl-defmethod-implicit-property-test \
               ((value integer) &optional fallback) \
               (if fallback fallback :integer))",
        )
        .unwrap_or_else(|error| panic!("second implicit DEFMETHOD failed: {error}"));
    let defaulted_call = runtime
        .eval(
            "(= (ncl-defmethod-implicit-property-test \
                  (make-condition 'ncl-defmethod-implicit-test-condition)) 47)",
        )
        .unwrap_or_else(|error| {
            panic!("implicit generic default-argument dispatch failed: {error}")
        });
    assert_eq!(defaulted_call, Word::TRUE);
    let optional_call = runtime
        .eval(
            "(= (ncl-defmethod-implicit-property-test \
                  (make-condition 'ncl-defmethod-implicit-test-condition) 83) 83)",
        )
        .unwrap_or_else(|error| {
            panic!("implicit generic optional-argument dispatch failed: {error}")
        });
    assert_eq!(optional_call, Word::TRUE);
    let second_method_call = runtime
        .eval("(eq (ncl-defmethod-implicit-property-test 83) :integer)")
        .unwrap_or_else(|error| panic!("existing generic dispatch failed: {error}"));
    assert_eq!(second_method_call, Word::TRUE);
    let first_method_retained = runtime
        .eval(
            "(= (ncl-defmethod-implicit-property-test \
                  (make-condition 'ncl-defmethod-implicit-test-condition)) 47)",
        )
        .unwrap_or_else(|error| panic!("first implicit method was lost: {error}"));
    assert_eq!(first_method_retained, Word::TRUE);
    let expander_property = runtime
        .eval("(not (null (get 'ncl-defmethod-implicit-property-test 'ncl::deftype-expander)))")
        .unwrap_or_else(|error| panic!("DEFTYPE expander property was lost: {error}"));
    assert_eq!(expander_property, Word::TRUE);
}

#[test]
fn defmethod_rejects_function_and_macro_name_collisions() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let function_collision = runtime
        .eval(
            "(progn \
               (defun ncl-defmethod-function-collision-test (value) value) \
               (handler-case \
                 (progn \
                   (defmethod ncl-defmethod-function-collision-test ((value integer)) value) \
                   nil) \
                 (program-error () (= (ncl-defmethod-function-collision-test 31) 31))))",
        )
        .unwrap_or_else(|error| panic!("function collision handler failed: {error}"));
    assert_eq!(function_collision, Word::TRUE);

    runtime
        .eval("(defmacro ncl-defmethod-macro-collision-test (value) value)")
        .unwrap_or_else(|error| panic!("DEFMACRO setup failed: {error}"));
    let macro_collision = runtime
        .eval(
            "(and \
               (handler-case \
                 (progn \
                   (defmethod ncl-defmethod-macro-collision-test ((value integer)) value) \
                   nil) \
                 (program-error () t)) \
               (not (null (macro-function 'ncl-defmethod-macro-collision-test))))",
        )
        .unwrap_or_else(|error| panic!("macro collision handler failed: {error}"));
    assert_eq!(macro_collision, Word::TRUE);
}

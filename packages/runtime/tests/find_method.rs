#![allow(missing_docs)]

use ncl_runtime::Runtime;
use ncl_sys::Word;

#[test]
fn find_method_returns_and_replaces_the_registered_method() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    runtime
        .eval("(defgeneric ncl-find-method-class-test (value))")
        .unwrap_or_else(|error| panic!("DEFGENERIC evaluation failed: {error}"));
    runtime
        .eval(
            "(defparameter *ncl-find-method-class-test-function* \
                 #'ncl-find-method-class-test)",
        )
        .unwrap_or_else(|error| panic!("generic function capture failed: {error}"));
    runtime
        .eval(
            "(defparameter *ncl-find-method-class-test-method* \
               (defmethod ncl-find-method-class-test ((value integer)) 17))",
        )
        .unwrap_or_else(|error| panic!("DEFMETHOD evaluation failed: {error}"));
    let is_method = runtime
        .eval("(typep *ncl-find-method-class-test-method* 'method)")
        .unwrap_or_else(|error| panic!("method TYPEP evaluation failed: {error}"));
    assert_eq!(is_method, Word::TRUE);
    let lookup = runtime
        .eval(
            "(and (eq *ncl-find-method-class-test-function* \
                     #'ncl-find-method-class-test) \
                  (eq *ncl-find-method-class-test-method* \
                      (find-method *ncl-find-method-class-test-function* nil \
                                   (list (find-class 'integer)))))",
        )
        .unwrap_or_else(|error| panic!("FIND-METHOD evaluation failed: {error}"));
    assert_eq!(lookup, Word::TRUE);

    runtime
        .eval(
            "(defparameter *ncl-find-method-class-test-old* \
                 *ncl-find-method-class-test-method*)",
        )
        .unwrap_or_else(|error| panic!("old method setup failed: {error}"));
    runtime
        .eval(
            "(setf *ncl-find-method-class-test-method* \
                 (defmethod ncl-find-method-class-test ((value integer)) 18))",
        )
        .unwrap_or_else(|error| panic!("method replacement failed: {error}"));
    let replaced = runtime
        .eval(
            "(and (eq *ncl-find-method-class-test-function* \
                     #'ncl-find-method-class-test) \
                  (not (eq *ncl-find-method-class-test-old* \
                           *ncl-find-method-class-test-method*)) \
                  (eq *ncl-find-method-class-test-method* \
                      (find-method *ncl-find-method-class-test-function* nil \
                                   (list (find-class 'integer)))))",
        )
        .unwrap_or_else(|error| panic!("replacement lookup failed: {error}"));
    assert_eq!(replaced, Word::TRUE);
}

#[test]
fn unspecialized_required_parameter_defaults_to_t_and_keeps_optional_arguments() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    runtime
        .eval(
            "(defgeneric ncl-default-t-specializer-test \
               (value &optional optional))",
        )
        .unwrap_or_else(|error| panic!("DEFGENERIC evaluation failed: {error}"));
    runtime
        .eval(
            "(defmethod ncl-default-t-specializer-test \
               (value &optional optional) (list value optional))",
        )
        .unwrap_or_else(|error| panic!("unspecialized DEFMETHOD evaluation failed: {error}"));
    let result = runtime
        .eval(
            "(and \
               (typep (find-method #'ncl-default-t-specializer-test nil \
                                   (list (find-class 't))) 'method) \
               (equal (ncl-default-t-specializer-test :first) \
                      (list :first nil)) \
               (equal (ncl-default-t-specializer-test :first 2) \
                      (list :first 2)))",
        )
        .unwrap_or_else(|error| panic!("unspecialized method lookup or dispatch failed: {error}"));
    assert_eq!(result, Word::TRUE);
}

#[test]
fn find_method_observes_errorp_and_checks_required_specializer_count() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    runtime
        .eval("(defgeneric ncl-find-method-missing-test (value))")
        .unwrap_or_else(|error| panic!("DEFGENERIC evaluation failed: {error}"));
    let missing_nil = runtime
        .eval(
            "(find-method #'ncl-find-method-missing-test nil \
                          (list (find-class 'integer)) nil)",
        )
        .unwrap_or_else(|error| panic!("ERRORP NIL lookup failed: {error}"));
    assert_eq!(missing_nil, Word::NIL);
    assert!(
        runtime
            .eval(
                "(find-method #'ncl-find-method-missing-test nil \
                          (list (find-class 'integer)))",
            )
            .is_err()
    );
    assert!(
        runtime
            .eval("(find-method #'ncl-find-method-missing-test nil nil nil)")
            .is_err()
    );
}

#[test]
fn find_method_matches_eql_specializers_and_qualifier_lists() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    runtime
        .eval(
            "(progn \
               (defgeneric ncl-find-method-eql-test (value)) \
               (defgeneric ncl-find-method-qualifier-test (value)) \
               (defparameter *ncl-find-method-eql-test-method* \
                 (defmethod ncl-find-method-eql-test ((value (eql 17))) 17)) \
               (defparameter *ncl-find-method-qualifier-test-method* \
                 (defmethod ncl-find-method-qualifier-test :before \
                            ((value integer)) nil)))",
        )
        .unwrap_or_else(|error| panic!("method setup failed: {error}"));
    let result = runtime
        .eval(
            "(and (eq *ncl-find-method-eql-test-method* \
                      (find-method #'ncl-find-method-eql-test nil (list #(1 17)))) \
                  (eq nil \
                      (find-method #'ncl-find-method-eql-test nil (list #(1 18)) nil)) \
                  (eq *ncl-find-method-qualifier-test-method* \
                      (find-method #'ncl-find-method-qualifier-test \
                                   '(:before) (list (find-class 'integer)))) \
                  (eq nil \
                      (find-method #'ncl-find-method-qualifier-test \
                                   '(:after) (list (find-class 'integer)) nil)))",
        )
        .unwrap_or_else(|error| panic!("FIND-METHOD specializer lookup failed: {error}"));
    assert_eq!(result, Word::TRUE);
}

#[test]
fn find_method_uses_generic_function_identity_and_signals_type_error() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let identity = runtime
        .eval(
            "(progn \
               (defgeneric ncl-find-method-identity-a (value)) \
               (defgeneric ncl-find-method-identity-b (value)) \
               (and \
                 (eq (defmethod ncl-find-method-identity-a ((value integer)) :a) \
                     (find-method #'ncl-find-method-identity-a nil \
                                  (list (find-class 'integer)))) \
                 (eq (defmethod ncl-find-method-identity-b ((value integer)) :b) \
                     (find-method #'ncl-find-method-identity-b nil \
                                  (list (find-class 'integer)))) \
                 (not (eq #'ncl-find-method-identity-a \
                          #'ncl-find-method-identity-b))))",
        )
        .unwrap_or_else(|error| panic!("generic-function identity lookup failed: {error}"));
    assert_eq!(identity, Word::TRUE);
    let type_error = runtime
        .eval(
            "(handler-case \
                 (progn (find-method #'identity nil nil) nil) \
               (type-error (condition) \
                 (and (eq (type-error-datum condition) #'identity) \
                      (eq (type-error-expected-type condition) 'generic-function))))",
        )
        .unwrap_or_else(|error| panic!("FIND-METHOD type error handling failed: {error}"));
    assert_eq!(type_error, Word::TRUE);
}

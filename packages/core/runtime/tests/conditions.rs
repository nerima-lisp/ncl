use ncl_runtime::{Runtime, Value};

fn evaluate(source: &str) -> Value {
    Runtime::new()
        .eval_source(source)
        .unwrap()
        .last()
        .cloned()
        .unwrap()
}

fn evaluate_compiled(source: &str) -> Value {
    Runtime::new()
        .eval_compiled_source(source)
        .expect("compiled evaluation should succeed")
        .last()
        .cloned()
        .expect("compiled source should return a value")
}

fn assert_both(source: &str, expected: &str) {
    assert_eq!(evaluate(source).to_string(), expected);
    assert_eq!(evaluate_compiled(source).to_string(), expected);
}

#[test]
fn define_condition_supports_inheritance_slots_accessors_and_report() {
    assert_both(
        r#"(progn
             (define-condition parent-condition (condition)
               ((parent-value
                 :initarg :parent-value
                 :initform 7
                 :reader parent-value)))
             (define-condition child-condition (parent-condition)
               ((child-value
                 :initarg :child-value
                 :initform (+ 1 2)
                 :accessor child-value))
               (:report "child report"))
             (let ((condition (make-condition 'child-condition
                                               :parent-value 11
                                               :child-value 13)))
               (list (parent-value condition)
                     (child-value condition)
                     (typep condition 'parent-condition)
                     (typep condition 'child-condition)
                     (typep condition 'condition)
                     (typep 42 'parent-condition)
                     (write-to-string condition))))"#,
        r##"(11 13 T T T NIL "#<CONDITION child report>")"##,
    );
}

#[test]
fn define_condition_supports_setf_writer_and_format_arguments() {
    assert_both(
        r#"(progn
             (define-condition mutable-condition (condition)
               ((value
                 :initarg :value
                 :initform 1
                 :accessor mutable-value
                 :writer set-mutable-value)))
             (let ((condition (make-condition 'mutable-condition)))
               (setf (mutable-value condition) 9)
               (set-mutable-value 11 condition)
               (list (mutable-value condition)
                     (write-to-string
                       (make-condition 'mutable-condition
                                        :format-control "value ~A"
                                        :format-arguments '(42))))))"#,
        r##"(11 "#<CONDITION value 42>")"##,
    );
}

#[test]
fn condition_types_work_with_typecase_the_and_subtypep() {
    assert_both(
        r#"(progn
             (define-condition parent-condition (condition) ())
             (define-condition child-condition (parent-condition) ())
             (let ((condition (make-condition 'child-condition)))
               (list
                 (typecase condition
                   (child-condition :child)
                   (parent-condition :parent)
                   (otherwise :other))
                 (typecase 42
                   (child-condition :child)
                   (otherwise :other))
                 (multiple-value-list
                   (subtypep 'child-condition 'parent-condition))
                 (ignore-errors (the child-condition 42)))))"#,
        "(:CHILD :OTHER (T T) NIL)",
    );
}

#[test]
fn make_condition_rejects_unknown_initargs_in_both_execution_modes() {
    let source = r#"(progn
                      (define-condition strict-condition (condition) ())
                      (make-condition 'strict-condition :unknown 1))"#;
    assert!(Runtime::new().eval_source(source).is_err());
    assert!(Runtime::new().eval_compiled_source(source).is_err());
}

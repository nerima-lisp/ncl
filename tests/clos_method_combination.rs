//! End-to-end tests for long-form custom method combinations.

#[path = "common/process.rs"]
mod process;

use std::process::{Command, Output};

fn run_eval(source: &str) -> Output {
    process::output(Command::new(env!("CARGO_BIN_EXE_ncl")).args(["--eval", source]))
}

#[test]
fn qualifier_groups_chain_around_and_primary_methods_in_specificity_order() {
    // CLHS 7.6.6.3 defines method-group selection and CALL-METHOD chaining.
    let source = r"
        (progn
          (define-method-combination chain-cases nil
            ((around-methods (:around)) (primary-methods))
            (let ((around (car around-methods)))
              (list 'loop
                    (list 'catch 'fail
                          (list 'return
                                (list 'rcase
                                      (list 1
                                            (list 'call-method around primary-methods))))))))
          (defgeneric chain-probe (value)
            (:method-combination chain-cases :most-specific-first))
          (defmethod chain-probe :around ((value integer))
            (list :around (call-next-method)))
          (defmethod chain-probe ((value integer))
            (list :integer (call-next-method)))
          (defmethod chain-probe ((value number)) :number)
          (chain-probe 1))
    ";
    let output = run_eval(source);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "(:AROUND (:INTEGER :NUMBER))"
    );
}

#[test]
fn method_groups_filter_qualifiers_and_skip_inapplicable_methods() {
    // CLHS 7.6.6.3 defines method-group qualifier and specializer matching.
    let source = r"
        (progn
          (define-method-combination exact-group nil
            ((selected (:pick)))
            (list 'loop
                  (list 'catch 'fail
                        (list 'return
                              (list 'rcase
                                    (list 1 (list 'call-method (car selected))))))))
          (defgeneric exact-group-probe (value)
            (:method-combination exact-group))
          (defmethod exact-group-probe :pick ((value integer)) :picked)
          (defmethod exact-group-probe :other ((value integer)) :other)
          (defmethod exact-group-probe :pick ((value string)) :wrong-specializer)
          (exact-group-probe 1))
    ";
    let output = run_eval(source);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), ":PICKED");
}

#[test]
fn standard_dispatch_orders_qualifiers_and_forwards_next_arguments() {
    // CLHS 7.6.6.2 specifies standard method order and call-next-method behavior.
    let source = r"
        (progn
          (defparameter *standard-dispatch-order* nil)
          (defgeneric standard-dispatch-probe (value))
          (defmethod standard-dispatch-probe :around ((value integer))
            (setq *standard-dispatch-order*
                  (cons :around-in *standard-dispatch-order*))
            (let ((result (call-next-method)))
              (setq *standard-dispatch-order*
                    (cons :around-out *standard-dispatch-order*))
              result))
          (defmethod standard-dispatch-probe :before ((value integer))
            (setq *standard-dispatch-order*
                  (cons :before-integer *standard-dispatch-order*)))
          (defmethod standard-dispatch-probe :before ((value number))
            (setq *standard-dispatch-order*
                  (cons :before-number *standard-dispatch-order*)))
          (defmethod standard-dispatch-probe ((value integer))
            (setq *standard-dispatch-order*
                  (cons :primary-integer *standard-dispatch-order*))
            (list :integer (next-method-p)
                  (call-next-method (+ value 1))))
          (defmethod standard-dispatch-probe ((value number))
            (setq *standard-dispatch-order*
                  (cons :primary-number *standard-dispatch-order*))
            (list :number (next-method-p) value))
          (defmethod standard-dispatch-probe :after ((value integer))
            (setq *standard-dispatch-order*
                  (cons :after-integer *standard-dispatch-order*)))
          (defmethod standard-dispatch-probe :after ((value number))
            (setq *standard-dispatch-order*
                  (cons :after-number *standard-dispatch-order*)))
          (let ((result (standard-dispatch-probe 1)))
            (list result (reverse *standard-dispatch-order*))))
    ";
    let output = run_eval(source);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "((:INTEGER T (:NUMBER NIL 2)) (:AROUND-IN :BEFORE-INTEGER :BEFORE-NUMBER :PRIMARY-INTEGER :PRIMARY-NUMBER :AFTER-NUMBER :AFTER-INTEGER :AROUND-OUT))"
    );
}

#[test]
fn and_method_combination_stops_after_the_first_nil_result() {
    // CLHS 7.6.6.4 defines AND method combination as short-circuiting at NIL.
    let source = r"
        (progn
          (defparameter *and-method-order* nil)
          (defgeneric short-and-probe (value)
            (:method-combination and))
          (defmethod short-and-probe and ((value integer))
            (setq *and-method-order* (cons :integer *and-method-order*))
            t)
          (defmethod short-and-probe and ((value number))
            (setq *and-method-order* (cons :number *and-method-order*))
            nil)
          (defmethod short-and-probe and ((value t))
            (setq *and-method-order* (cons :t *and-method-order*))
            :unreached)
          (and (null (short-and-probe 1))
               (eq (car *and-method-order*) :number)
               (eq (cadr *and-method-order*) :integer)
               (null (cddr *and-method-order*))))
    ";
    let output = run_eval(source);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "T");
}

#[test]
fn defgeneric_method_option_installs_a_callable_method() {
    // CLHS DEFGENERIC specifies inline :METHOD options.
    let source = r"
        (progn
          (defgeneric inline-method-probe (value)
            (:method ((value integer)) :from-defgeneric))
          (inline-method-probe 1))
    ";
    let output = run_eval(source);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        ":FROM-DEFGENERIC"
    );
}

#[test]
fn qualifier_predicates_select_the_weighted_method_at_each_boundary() {
    // CLHS DEFINE-METHOD-COMBINATION specifies predicate groups. CLHS RANDOM
    // returns a value below its limit, so a total weight of one fixes it at zero.
    let source = r"
        (progn
          (defun integer-qualifier-p (qualifiers)
            (typep qualifiers '(cons integer null)))
          (define-method-combination weighted-probe nil
            ((methods integer-qualifier-p))
            (let ((clauses
                    (mapcar #'(lambda (method)
                                (let ((weight (car (method-qualifiers method))))
                                  (list weight (list 'call-method method))))
                            methods)))
              (list 'loop
                    (list 'catch 'fail
                          (list 'return (cons 'rcase clauses))))))
          (defgeneric weighted-first (value)
            (:method-combination weighted-probe))
          (defmethod weighted-first 1 ((value integer)) :specific)
          (defmethod weighted-first 0 ((value t)) :general)
          (defgeneric weighted-last (value)
            (:method-combination weighted-probe))
          (defmethod weighted-last 0 ((value integer)) :specific)
          (defmethod weighted-last 1 ((value t)) :general)
          (list (weighted-first 1) (weighted-last 1)))
    ";
    let output = run_eval(source);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "(:SPECIFIC :GENERAL)"
    );
}

#[test]
fn malformed_long_form_method_combinations_are_rejected() {
    // CLHS 7.6.6.3 requires a valid method-group specifier and a body.
    let cases = [
        (
            "empty method-combination body",
            "(define-method-combination no-body nil ())",
        ),
        (
            "group without a variable",
            "(define-method-combination no-group nil (()))",
        ),
        (
            "non-symbol combination name",
            "(defgeneric invalid-combination (value) (:method-combination \"not-a-combination\"))",
        ),
        (
            "undefined custom combination",
            "(defgeneric unknown-combination (value) (:method-combination unknown-combination))",
        ),
        (
            "unsupported custom order",
            "(progn (define-method-combination ordered nil ((methods)) (first methods)) (defgeneric invalid-order (value) (:method-combination ordered :sideways)))",
        ),
    ];

    for (name, source) in cases {
        let output = run_eval(source);
        assert_eq!(output.status.code(), Some(1), "{name}: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("TypeError"),
            "{name}: {output:?}"
        );
    }
}

#[test]
fn method_qualifiers_rejects_values_that_are_not_methods() {
    // CLHS 7.7 defines METHOD-QUALIFIERS as a standard generic function on methods.
    for (name, source) in [
        ("integer", "(ncl-mop:method-qualifiers 42)"),
        (
            "short list",
            "(ncl-mop:method-qualifiers (list :not-a-method))",
        ),
    ] {
        let output = run_eval(source);
        assert_eq!(output.status.code(), Some(1), "{name}: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("TypeError"),
            "{name}: {output:?}"
        );
    }
}

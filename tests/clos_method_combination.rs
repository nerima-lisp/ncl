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
fn method_qualifiers_rejects_a_non_method_object() {
    // CLHS 7.7 defines METHOD-QUALIFIERS as a standard generic function on methods.
    let output = run_eval("(ncl-mop:method-qualifiers (list :not-a-method))");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("TypeError"),
        "{output:?}"
    );
}

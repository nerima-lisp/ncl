#![allow(missing_docs)]

use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const CHILD_TIMEOUT: Duration = Duration::from_secs(30);

struct Case {
    name: &'static str,
    source: &'static str,
    stdout: &'static str,
}

struct XFail {
    name: &'static str,
    source: &'static str,
    stderr: &'static str,
}

fn run_ncl(source: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", source])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("{source}: failed to run ncl: {error}"));
    let deadline = Instant::now() + CHILD_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .unwrap_or_else(|error| panic!("{source}: failed to collect output: {error}"));
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                child
                    .kill()
                    .unwrap_or_else(|error| panic!("{source}: failed to stop timeout: {error}"));
                let _ = child.wait();
                panic!("{source}: ncl did not exit within {CHILD_TIMEOUT:?}");
            }
            Err(error) => panic!("{source}: failed to poll ncl: {error}"),
        }
    }
}

#[test]
fn compiled_clos_baseline_asserts_ansi_output() {
    let cases = [
        Case {
            name: "class-of-integer",
            source: "(class-name (class-of 3))",
            stdout: "\"INTEGER\"",
        },
        Case {
            name: "class-of-nil",
            source: "(class-name (class-of nil))",
            stdout: "\"NULL\"",
        },
    ];
    for case in cases {
        let output = run_ncl(case.source);
        assert_eq!(output.status.code(), Some(0), "{}", case.name);
        assert!(
            output.stderr.is_empty(),
            "{}: {:?}",
            case.name,
            output.stderr
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim_end(),
            case.stdout,
            "{}",
            case.name
        );
    }
}

#[test]
fn compiled_clos_builtin_class_objects_have_their_metaclass() {
    let output = run_ncl(
        "(list (class-name (find-class 't)) (class-name (find-class 'nil)) (typep (class-of 1) 'built-in-class) (typep 1 'built-in-class) (typep (find-class 'integer) 'built-in-class) (typep (class-of 1) 'standard-object) (= (length (find-class 'ncl-ffi::integer)) 3))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        "(\"T\" \"NULL\" T NIL T T T)"
    );
}

#[test]
fn compiled_clos_class_designators_and_precedence_follow_supported_relations() {
    let output = run_ncl(
        "(let ((null-precedence (ncl-mop:class-precedence-list (find-class 'null)))) \
           (and \
             (eq (class-of (find-class 'integer)) (find-class 'built-in-class)) \
             (eq (class-of (find-class 'standard-object)) (find-class 'standard-class)) \
             (typep nil 'symbol) \
             (typep nil 'list) \
             (typep 1 'number) \
             (typep \"x\" 'vector) \
             (subtypep 'integer 'number) \
             (= (length null-precedence) 5) \
             (string= (class-name (svref null-precedence 0)) \"NULL\") \
             (string= (class-name (svref null-precedence 1)) \"SYMBOL\") \
             (string= (class-name (svref null-precedence 2)) \"LIST\") \
             (string= (class-name (svref null-precedence 3)) \"SEQUENCE\") \
             (string= (class-name (svref null-precedence 4)) \"T\") \
             (handler-case (progn (find-class (find-class 'ncl-ffi::integer)) nil) \
               (type-error () t))))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "T");
}

#[test]
fn compiled_clos_method_dispatch_uses_class_precedence_order() {
    let output = run_ncl(
        "(progn \
           (defclass precedence-left-probe () ()) \
           (defclass precedence-right-probe () ()) \
           (defclass precedence-child-probe \
             (precedence-left-probe precedence-right-probe) ()) \
           (defgeneric precedence-child-method (value)) \
           (defmethod precedence-child-method \
             ((value precedence-left-probe)) :left) \
           (defmethod precedence-child-method \
             ((value precedence-right-probe)) :right) \
           (defgeneric precedence-null-method (value)) \
           (defmethod precedence-null-method ((value symbol)) :symbol) \
           (defmethod precedence-null-method ((value list)) :list) \
           (let ((precedence \
                   (ncl-mop:class-precedence-list \
                     (find-class 'precedence-child-probe)))) \
             (and \
               (= (length precedence) 5) \
               (string= (class-name (svref precedence 1)) \
                        \"PRECEDENCE-LEFT-PROBE\") \
               (string= (class-name (svref precedence 2)) \
                        \"PRECEDENCE-RIGHT-PROBE\") \
               (eq (precedence-child-method \
                     (make-instance 'precedence-child-probe)) :left) \
               (eq (precedence-null-method nil) :symbol))))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "T");
}

#[test]
fn compiled_clos_generic_functions_have_the_standard_generic_function_class() {
    let output = run_ncl(
        "(progn \
           (defgeneric generic-class-probe (value)) \
           (and \
             (eq (class-of #'generic-class-probe) \
                 (find-class 'standard-generic-function)) \
             (typep #'generic-class-probe 'generic-function)))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "T");
}

#[test]
fn compiled_clos_classes_without_explicit_superclasses_default_to_standard_object() {
    let output = run_ncl(
        "(progn \
           (defclass default-root-probe () ()) \
           (typep (make-instance 'default-root-probe) 'standard-object))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "T");
}

#[test]
fn compiled_clos_defmethod_unspecialized_required_arguments_default_to_t() {
    let output = run_ncl(
        "(progn (defgeneric method-default-specializer (x y)) (defmethod method-default-specializer ((x integer) y) (list x y)) (method-default-specializer 2 :anything))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        "(2 :ANYTHING)"
    );
}

#[test]
fn unsupported_compiled_clos_cases_remain_explicit_xfails() {
    let cases = [
        XFail {
            name: "call-next-method-outside-method",
            source: "(call-next-method)",
            stderr: "UNDEFINED-FUNCTION",
        },
        XFail {
            name: "unknown-generic",
            source: "(clos-unknown-generic 1)",
            stderr: "UNDEFINED-FUNCTION",
        },
    ];
    for case in cases {
        let output = run_ncl(case.source);
        assert_eq!(output.status.code(), Some(1), "{}", case.name);
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(case.stderr),
            "{}: stderr={:?}",
            case.name,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn compiled_clos_call_next_method_combines_primary_methods() {
    let output = run_ncl(
        "(progn (defclass point () ()) (defclass colored (point) ()) (defgeneric area (s)) (defmethod area ((s point)) 2) (defmethod area ((s colored)) (+ 3 (call-next-method))) (area (make-instance 'colored)))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "5");
}

#[test]
fn compiled_clos_method_combination_and_next_method_p_are_observable() {
    let output = run_ncl(
        "(progn (defgeneric combine (x)) (defmethod combine :around ((x integer)) (+ 100 (call-next-method))) (defmethod combine :before ((x integer)) (set 'clos-before 4)) (defmethod combine :after ((x integer)) (set 'clos-after 8)) (defmethod combine ((x integer)) (if (next-method-p) 0 3)) (list (combine 1) clos-before clos-after))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        "(103 4 8)"
    );
}

#[test]
fn compiled_clos_class_and_instance_paths_are_available() {
    let output = run_ncl(
        "(progn (defclass point () ((x :initarg :x :accessor px) (y :initarg :y :initform 0 :accessor py))) (defclass colored (point) ()) (defgeneric area (s)) (defmethod area ((s point)) (* (px s) (py s))) (area (make-instance 'colored :x 3)))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "0");
}

#[test]
fn compiled_clos_quote_keeps_call_next_method_as_data() {
    let output = run_ncl(
        "(progn (defgeneric quoted-probe (x)) (defmethod quoted-probe ((x integer)) (car (list 'call-next-method))) (quoted-probe 1))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        "COMMON-LISP-USER:CALL-NEXT-METHOD"
    );
}

#[test]
fn compiled_clos_throw_passes_through_call_next_method() {
    let output = run_ncl(
        "(catch 'k (progn (defgeneric throw-probe (s)) (defmethod throw-probe ((s integer)) (throw 'k 42)) (defmethod throw-probe :around ((s integer)) (+ 1000 (call-next-method))) (throw-probe 3)))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "42");
}

#[test]
fn compiled_clos_slot_names_boundp_typep_and_makunbound_are_observable() {
    let output = run_ncl(
        "(progn (defclass slot-probe () ((value :initarg :value))) (let ((object (make-instance 'slot-probe :value 9))) (list (slot-exists-p object 'value) (slot-boundp object 'value) (typep object 'slot-probe) (progn (slot-makunbound object 'value) (slot-boundp object 'value)))))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        "(T T T NIL)"
    );
}

#[test]
fn compiled_clos_defmethod_rest_arguments_are_dispatched() {
    let output = run_ncl(
        "(progn (defclass method-rest-probe () ()) (defgeneric method-rest-values (object &rest values)) (defmethod method-rest-values ((object method-rest-probe) &rest values) values) (method-rest-values (make-instance 'method-rest-probe) 1 2))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "(1 2)");
}

#[test]
fn compiled_clos_defmethod_optional_arguments_are_dispatched() {
    let output = run_ncl(
        "(progn (defclass method-optional-probe () ()) (defgeneric method-optional-value (object &optional value)) (defmethod method-optional-value ((object method-optional-probe) &optional value) (if value value 7)) (let ((object (make-instance 'method-optional-probe))) (list (method-optional-value object) (method-optional-value object 9))))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "(7 9)");
}

#[test]
fn compiled_clos_defmethod_key_arguments_are_dispatched() {
    let output = run_ncl(
        "(progn (defclass method-key-probe () ()) (defgeneric method-key-value (object &key value)) (defmethod method-key-value ((object method-key-probe) &key value) value) (method-key-value (make-instance 'method-key-probe) :value 9))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "9");
}

#[test]
fn compiled_clos_shared_initialize_preserves_bound_slots() {
    let output = run_ncl(
        "(progn (defclass shared-init-probe () ((value :initform 7 :initarg :value :accessor shared-init-value))) (let ((object (make-instance 'shared-init-probe :value 9))) (shared-initialize object t) (shared-init-value object)))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "9");
}

#[test]
fn compiled_clos_shared_initialize_limits_initforms_to_slot_names() {
    let output = run_ncl(
        "(progn (defclass shared-init-slot-names-probe () ((first :initform 7) (second :initform 9))) (let ((object (make-instance 'shared-init-slot-names-probe))) (slot-makunbound object 'first) (slot-makunbound object 'second) (shared-initialize object '(first)) (list (slot-boundp object 'first) (slot-boundp object 'second) (slot-value object 'first))))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        "(T NIL 7)"
    );
}

#[test]
fn compiled_clos_explicit_call_next_method_arguments_are_rewritten() {
    let output = run_ncl(
        "(progn (defgeneric explicit-next (value)) (defmethod explicit-next ((value integer)) value) (defmethod explicit-next :around ((value integer)) (+ 10 (call-next-method 4))) (explicit-next 2))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "14");
}

#[test]
fn compiled_clos_implicit_call_next_forwards_rest_arguments() {
    let output = run_ncl(
        "(progn (defclass implicit-rest-parent () ()) (defclass implicit-rest-child (implicit-rest-parent) ()) (defgeneric implicit-rest-values (object &rest values)) (defmethod implicit-rest-values ((object implicit-rest-child) &rest values) (call-next-method)) (defmethod implicit-rest-values ((object implicit-rest-parent) &rest values) values) (implicit-rest-values (make-instance 'implicit-rest-child) 1 2))",
    );
    assert_eq!(output.status.code(), Some(0), "stderr={:?}", output.stderr);
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "(1 2)");
}

#[test]
fn compiled_clos_implicit_call_next_forwards_optional_arguments() {
    let output = run_ncl(
        "(progn (defclass implicit-optional-parent () ()) (defclass implicit-optional-child (implicit-optional-parent) ()) (defgeneric implicit-optional-value (object &optional value)) (defmethod implicit-optional-value ((object implicit-optional-child) &optional value) (call-next-method)) (defmethod implicit-optional-value ((object implicit-optional-parent) &optional value) value) (implicit-optional-value (make-instance 'implicit-optional-child) 9))",
    );
    assert_eq!(output.status.code(), Some(0), "stderr={:?}", output.stderr);
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "9");
}

#[test]
fn compiled_clos_implicit_call_next_forwards_key_arguments() {
    let output = run_ncl(
        "(progn (defclass implicit-key-parent () ()) (defclass implicit-key-child (implicit-key-parent) ()) (defgeneric implicit-key-value (object &key value)) (defmethod implicit-key-value ((object implicit-key-child) &key value) (call-next-method)) (defmethod implicit-key-value ((object implicit-key-parent) &key value) value) (implicit-key-value (make-instance 'implicit-key-child) :value 9))",
    );
    assert_eq!(output.status.code(), Some(0), "stderr={:?}", output.stderr);
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "9");
}

#[test]
fn compiled_clos_implicit_call_next_forwards_latest_explicit_arguments() {
    let output = run_ncl(
        "(progn (defclass implicit-next-base () ()) (defclass implicit-next-parent (implicit-next-base) ()) (defclass implicit-next-child (implicit-next-parent) ()) (defgeneric implicit-next-values (object &rest values)) (defmethod implicit-next-values ((object implicit-next-child) &rest values) (call-next-method object 3 4)) (defmethod implicit-next-values ((object implicit-next-parent) &rest values) (call-next-method)) (defmethod implicit-next-values ((object implicit-next-base) &rest values) values) (implicit-next-values (make-instance 'implicit-next-child) 1 2))",
    );
    assert_eq!(output.status.code(), Some(0), "stderr={:?}", output.stderr);
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "(3 4)");
}

#[test]
fn compiled_clos_class_of_and_empty_class_definition_are_observable() {
    let output = run_ncl(
        "(progn (defclass empty-probe () ()) (list (class-name (find-class 'empty-probe)) (class-name (class-of (cons 1 2))) (class-name (class-of 1.0))))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        "(COMMON-LISP-USER:EMPTY-PROBE \"CONS\" \"DOUBLE-FLOAT\")"
    );
}

#[test]
fn compiled_clos_invalid_slot_and_initargs_report_errors() {
    let cases = [
        ("slot-value-non-instance", "(slot-value nil 0)", "TypeError"),
        (
            "slot-value-unknown-name",
            "(slot-value (make-instance 'standard-object) 'missing-slot)",
            "TypeError",
        ),
        (
            "make-instance-odd-initargs",
            "(make-instance 'standard-object :unexpected)",
            "TypeError",
        ),
        (
            "mop-slot-definition-type",
            "(ncl-mop:slot-definition-name 0)",
            "TypeError",
        ),
        (
            "mop-class-type",
            "(ncl-mop:class-direct-superclasses 0)",
            "TypeError",
        ),
    ];
    for (name, source, error) in cases {
        let output = run_ncl(source);
        assert_eq!(output.status.code(), Some(1), "{name}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(error),
            "{name}: stderr={:?}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn compiled_clos_mop_class_and_slot_metadata_are_observable() {
    let output = run_ncl(
        "(progn (defclass mop-probe () ((value :initarg :value))) (let* ((class (find-class 'mop-probe)) (slot (svref (ncl-mop:class-slots class) 0))) (list (length (ncl-mop:class-precedence-list class)) (length (ncl-mop:class-direct-superclasses class)) (length (ncl-mop:class-direct-slots class)) (ncl-mop:class-finalized-p class) (ncl-mop:slot-definition-name slot))))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        "(1 0 1 T COMMON-LISP-USER:VALUE)"
    );
}

#[test]
fn compiled_clos_mop_inherited_slot_metadata_is_observable() {
    let output = run_ncl(
        "(progn (defclass mop-parent () ((parent-value :initarg :parent-value))) (defclass mop-child (mop-parent) ((child-value :initarg :child-value))) (let* ((class (find-class 'mop-child)) (direct (ncl-mop:class-direct-superclasses class)) (slots (ncl-mop:class-slots class))) (list (length slots) (length direct) (class-name (svref direct 0)))))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        "(2 1 COMMON-LISP-USER:MOP-PARENT)"
    );
}

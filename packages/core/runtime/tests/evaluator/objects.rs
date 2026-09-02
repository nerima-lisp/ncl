use super::*;

#[test]
fn evaluates_basic_clos_instances_and_accessors() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass point ()
                   ((x :initarg :x :accessor point-x)
                    (y :initarg :y :accessor point-y)))
                 (let ((point (make-instance 'point :x 2 :y 3)))
                   (list (slot-value point 'x)
                         (point-x point)
                         (point-y point)
                         (slot-exists-p point 'x)
                         (slot-boundp point 'y)
                         (typep point 'point)
                         (class-name (class-of point))
                         (class-name (find-class 'point)))))"#,
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(2 2 3 T T T POINT POINT)");
}

#[test]
fn evaluates_reinitialize_instance() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass reinit-point ()
                   ((x :initarg :x :initform 1)
                    (y :initarg :y :initform 2)))
                 (let ((point (make-instance 'reinit-point :x 10 :y 20)))
                   (reinitialize-instance point :x 30)
                   (list (slot-value point 'x) (slot-value point 'y))))"#,
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(30 20)");
}

#[test]
fn evaluates_initialize_instance() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass initialize-point ()
                   ((x :initarg :x :initform 1)))
                 (let ((point (make-instance 'initialize-point)))
                   (initialize-instance point :x 8)
                   (slot-value point 'x)))"#,
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "8");
}

#[test]
fn evaluates_class_default_initargs() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass default-initarg-point ()
                   ((x :initarg :x))
                   (:default-initargs :x 42))
                 (class-default-initargs (find-class 'default-initarg-point)))"#,
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "((X . 42))");
}

#[test]
fn evaluates_class_finalized_p() {
    let values = Runtime::new()
        .eval_source("(progn (defclass finalized-class () ()) (class-finalized-p (find-class 'finalized-class)))")
        .must_exist();
    assert_eq!(values[0].to_string(), "T");
}

#[test]
fn evaluates_class_direct_default_initargs() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass direct-default-initarg-parent () () (:default-initargs :parent 1))
                 (defclass direct-default-initarg-child (direct-default-initarg-parent) ()
                   (:default-initargs :child 2))
                 (list
                   (class-direct-default-initargs (find-class 'direct-default-initarg-child))
                   (class-default-initargs (find-class 'direct-default-initarg-child))))"#,
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "(((CHILD . 2)) ((CHILD . 2) (PARENT . 1)))"
    );
}

#[test]
fn rejects_invalid_make_instance_arguments() {
    for source in [
        "(make-instance)",
        "(make-instance 'point :x)",
        "(make-instance 1)",
        "(make-instance 'missing-class-for-test)",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_setf_slot_value() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass setf-point ()
                   ((x :initarg :x)))
                 (let ((point (make-instance 'setf-point :x 2)))
                   (setf (slot-value point 'x) 9)
                   (slot-value point 'x)))"#,
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "9");
}

#[test]
fn evaluates_clos_with_slots_and_accessors() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_source(
            r"(progn
                 (defclass ws-point ()
                   ((x :initarg :x :accessor ws-point-x)
                    (y :initarg :y :accessor ws-point-y)))
                 (let ((point (make-instance 'ws-point :x 2 :y 3)))
                   (with-slots ((x xx) y) point
                     (setf xx 5 y 7)
                     (with-accessors ((ax ws-point-x) (ay ws-point-y)) point
                       (list xx y ax ay
                             (progn (setf ax 11) ax)
                             (ws-point-x point)
                             (ws-point-y point))))))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(5 7 5 7 11 11 7)");

    assert!(runtime
        .eval_source("(with-accessors (x) object x)")
        .is_err());
}

#[test]
fn evaluates_clos_slot_initialization_options() {
    let values = Runtime::new()
        .eval_source(
            r"(progn
                 (defclass defaults ()
                   ((x :initform 7 :reader defaults-x)
                    (y :initarg :y :writer set-defaults-y)
                    (z :initarg nil)))
                 (let ((object (make-instance 'defaults :y 3)))
                   (set-defaults-y 9 object)
                   (list (defaults-x object)
                         (slot-value object 'y)
                         (slot-boundp object 'z)
                         (not (ignore-errors (make-instance 'defaults :x 1))))))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(7 9 NIL T)");
}

#[test]
fn evaluates_clos_class_allocated_slots() {
    let values = Runtime::new()
        .eval_source(
            r"(progn
                 (defclass counter ()
                   ((value :allocation :class :initarg :value
                           :initform 0 :accessor counter-value)))
                 (defclass child-counter (counter) ())
                 (let ((counter (make-instance 'counter))
                       (child (make-instance 'child-counter :value 4)))
                   (setf (counter-value counter) 7)
                   (let ((before (list (counter-value counter)
                                       (counter-value child)
                                       (slot-boundp counter 'value)
                                       (slot-boundp child 'value))))
                     (slot-makunbound child 'value)
                     (list before
                           (slot-boundp counter 'value)
                           (slot-boundp child 'value)))))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "((7 7 T T) NIL NIL)");
}

#[test]
fn evaluates_clos_class_allocated_slot_reuse_without_reinitializing() {
    let values = Runtime::new()
        .eval_source(
            r"(progn
                 (defclass shared-counter ()
                   ((value :allocation :class :initform 0 :accessor shared-counter-value)))
                 (make-instance 'shared-counter)
                 (setf (shared-counter-value (make-instance 'shared-counter)) 5)
                 (list (shared-counter-value (make-instance 'shared-counter))))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(5)");
}

#[test]
fn evaluates_clos_default_initargs() {
    let values = Runtime::new()
        .eval_source(
            r"(progn
                 (defclass defaults ()
                   ((value :initarg :value :initform 1))
                   (:default-initargs :value (+ 2 5)))
                 (defclass child-defaults (defaults) ())
                 (let ((explicit (make-instance 'child-defaults :value 9))
                       (implicit (make-instance 'child-defaults)))
                   (list (slot-value explicit 'value)
                         (slot-value implicit 'value))))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(9 7)");
}

#[test]
fn evaluates_clos_setf_and_generic_methods() {
    let values = Runtime::new()
        .eval_source(
            r"(progn
                 (defclass point ()
                   ((x :initarg :x :accessor point-x)
                    (y :initarg :y :accessor point-y)))
                 (defgeneric point-total (object))
                 (defmethod point-total ((object point))
                   (+ (point-x object) (point-y object)))
                 (let ((point (make-instance 'point :x 2 :y 3)))
                   (setf (point-x point) 8)
                   (list (point-x point)
                         (slot-value point 'x)
                         (point-total point))))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(8 8 11)");
}

#[test]
fn evaluates_clos_methods_with_ordinary_lambda_lists() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass point () ())
                 (defgeneric describe-point (object &optional prefix &key suffix))
                 (defmethod describe-point
                   ((object point)
                    &optional (prefix "default" prefix-p)
                    &key (suffix "suffix" suffix-p))
                   (list prefix suffix prefix-p suffix-p))
                 (defgeneric collect-point (object))
                 (defmethod collect-point ((object point) &rest values)
                   values)
                 (let ((point (make-instance 'point)))
                   (list (describe-point point)
                         (describe-point point "given" :suffix "tail")
                         (collect-point point 1 2 3))))"#,
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "((\"default\" \"suffix\" NIL NIL) (\"given\" \"tail\" T T) (1 2 3))"
    );
}

#[test]
fn evaluates_clos_inheritance_and_specialization() {
    let values = Runtime::new()
        .eval_source(
            r"(progn
                 (defclass point ()
                   ((x :initarg :x :accessor point-x)))
                 (defclass colored-point (point)
                   ((color :initarg :color :accessor point-color)))
                 (defgeneric point-coordinate (object))
                 (defmethod point-coordinate ((object point))
                   (list (point-x object)))
                 (let ((point (make-instance 'colored-point :x 4 :color :red)))
                   (list (point-x point)
                         (point-color point)
                         (typep point 'point)
                         (typep point 'colored-point)
                         (point-coordinate point))))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(4 :RED T T (4))");
}

#[test]
fn evaluates_clos_unbound_slots() {
    let values = Runtime::new()
        .eval_source(
            r"(progn
                 (defclass point ()
                   ((x :initarg :x)))
                 (let ((point (make-instance 'point)))
                   (list
                     (slot-exists-p point 'x)
                     (slot-boundp point 'x)
                     (not (ignore-errors (slot-value point 'x)))
                     (progn
                       (setf (slot-value point 'x) 9)
                       (list (slot-boundp point 'x) (slot-value point 'x)))
                     (progn
                       (slot-makunbound point 'x)
                       (slot-boundp point 'x)))))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(T NIL T (T 9) NIL)");
}

#[test]
fn rejects_setting_an_undefined_clos_slot() {
    let result = Runtime::new().eval_source(
        r"(progn
             (defclass point () ((x)))
             (setf (slot-value (make-instance 'point) 'missing) 9))",
    );
    assert!(result.is_err());
}

#[test]
fn enforces_clos_slot_types_on_initialization_and_writes() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass typed-point () ((x :initarg :x :type integer)))
                 (let ((point (make-instance 'typed-point :x 1)))
                   (list (slot-value point 'x)
                         (not (ignore-errors (setf (slot-value point 'x) "bad")))
                         (not (ignore-errors (make-instance 'typed-point :x "bad"))))))"#,
        )
        .must_exist();
    assert_eq!(values[0].to_string(), "(1 T T)");
}

#[test]
fn evaluates_clos_method_combination() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defclass point () ((x :initarg :x)))
                 (let ((events nil))
                   (defgeneric point-value (object))
                   (defmethod point-value ((object t))
                     (setf events (cons :base events))
                     (list :base (next-method-p)))
                   (defmethod point-value :before ((object point))
                     (setf events (cons :before events)))
                   (defmethod point-value :after ((object point))
                     (setf events (cons :after events)))
                   (defmethod point-value ((object point))
                     (setf events (cons :primary events))
                     (list :primary (next-method-p) (call-next-method)))
                   (defmethod point-value :around ((object point))
                     (setf events (cons :around-before events))
                     (let ((value (call-next-method)))
                       (setf events (cons :around-after events))
                       (list :around value)))
                   (let ((point (make-instance 'point :x 7)))
                     (list (point-value point) (reverse events)))))"#,
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "((:AROUND (:PRIMARY T (:BASE NIL))) (:AROUND-BEFORE :BEFORE :PRIMARY :BASE :AFTER :AROUND-AFTER))"
    );
}

#[test]
fn rejects_invalid_eval_when_situations() {
    for source in [
        "(eval-when 1)",
        "(eval-when (1) 42)",
        "(eval-when (#\\a) 42)",
        "(eval-when (#:uninterned) 42)",
    ] {
        let error = Runtime::new().eval_source(source).must_fail();
        assert!(
            matches!(error, ncl_runtime::RuntimeError::InvalidForm { .. }),
            "{source}: {error:?}"
        );
    }
}

#[test]
fn evaluates_defstruct_constructors_accessors_and_copies() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defstruct person name (age 21))
                 (let ((person (make-person :name "Ada")))
                   (setf (person-name person) "Grace")
                   (list (person-p person)
                         (person-name person)
                         (person-age person)
                         (typep person 'person)
                         (type-of person)
                         (eq person (copy-person person))
                         (equal person (copy-person person))
                         (write-to-string person))))"#,
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        r##"(T "Grace" 21 T PERSON NIL T "#S(PERSON :NAME \"Grace\" :AGE 21)")"##,
    );
}

#[test]
fn evaluates_defstruct_name_and_options() {
    let values = Runtime::new()
        .eval_source(
            r"(progn
                 (defstruct
                   (account
                    (:conc-name acct-)
                    (:predicate account-p)
                    (:copier clone-account)
                    (:constructor make-account-record))
                   id (balance 0))
                 (defstruct
                   (plain
                    (:conc-name nil)
                    (:predicate plain-p)
                    (:copier clone-plain)
                    (:constructor make-plain))
                   amount)
                 (defstruct
                   (disabled
                    (:predicate nil)
                    (:copier nil)
                    (:constructor nil))
                   value)
                 (let ((account (make-account-record :id 7))
                       (plain (make-plain :amount 9)))
                   (list (account-p account)
                         (acct-id account)
                         (acct-balance account)
                         (equal account (clone-account account))
                         (typep account 'account)
                         (type-of account)
                         (amount plain)
                         (plain-p plain)
                         (equal plain (clone-plain plain)))))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(T 7 0 T T ACCOUNT 9 T T)");
}

#[test]
fn evaluates_defstruct_read_only_slots() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defstruct record
                   (id 0 t)
                   (label "initial" nil))
                 (let ((record (make-record :id 7 :label "before")))
                   (setf (record-label record) "after")
                   (list (record-id record)
                         (record-label record))))"#,
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), r#"(7 "after")"#);

    let error = Runtime::new()
        .eval_source(
            r"(progn
                 (defstruct immutable (id 0 t))
                 (let ((record (make-immutable :id 1)))
                   (setf (immutable-id record) 2)))",
        )
        .must_fail();
    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::InvalidForm { message, .. }
            if message == "cannot SETF a read-only structure slot"
    ));
}

#[test]
fn evaluates_defstruct_included_slots_and_type_hierarchy() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
                 (defstruct
                   (person (:constructor nil))
                   name (age 0))
                 (defstruct
                   (employee (:include person (age 21)))
                   id)
                 (let ((employee (make-employee :name "Ada" :id 7)))
                   (setf (person-name employee) "Grace")
                   (setf (employee-age employee) 42)
                   (list (employee-name employee)
                         (person-name employee)
                         (employee-age employee)
                         (person-age employee)
                         (employee-id employee)
                         (person-p employee)
                         (employee-p employee)
                         (typep employee 'person)
                         (typep employee 'employee)
                         (type-of employee)
                         (equal employee (copy-person employee))
                         (write-to-string employee))))"#,
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        r##"("Grace" "Grace" 42 42 7 T T T T EMPLOYEE T "#S(EMPLOYEE :NAME \"Grace\" :AGE 42 :ID 7)")"##,
    );
}

#[test]
fn evaluates_defstruct_boa_constructors() {
    let values = Runtime::new()
        .eval_source(
            r"(progn
                 (defstruct
                   (boa
                    (:constructor make-boa
                      (first
                       &optional
                       (second)
                       (third (+ first second))
                       &rest rest
                       &key
                       ((:flag flag) t)
                       &aux
                       (sum (+ first second)))))
                   first (second 20) (third 30) rest flag sum)
                 (let ((default (make-boa 1))
                       (explicit (make-boa 1 2 3 :flag nil)))
                   (list
                     (list (boa-first default)
                           (boa-second default)
                           (boa-third default)
                           (boa-rest default)
                           (boa-flag default)
                           (boa-sum default))
                     (list (boa-first explicit)
                           (boa-second explicit)
                           (boa-third explicit)
                           (boa-rest explicit)
                           (boa-flag explicit)
                           (boa-sum explicit)))))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "((1 20 21 NIL T 21) (1 2 3 (:FLAG NIL) NIL 3))",
    );
}

#[test]
fn evaluates_defstruct_boa_argument_errors() {
    let cases = [
        r"(progn (defstruct (record (:constructor make-record (required))) required) (make-record))",
        r"(progn (defstruct (record (:constructor make-record (required))) required) (make-record 1 2))",
        r"(progn (defstruct (record (:constructor make-record (required &key flag))) required flag) (make-record 1 2))",
        r"(progn (defstruct (record (:constructor make-record (required &key flag))) required flag) (make-record 1 :unknown 2))",
    ];

    for source in cases {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn rejects_malformed_structure_and_class_definitions() {
    let cases = [
        (
            "defstruct option must be a list",
            r"(defstruct record 1 value)",
        ),
        (
            "defstruct option needs a name",
            r"(defstruct record () value)",
        ),
        (
            "defstruct naming option has too many names",
            r"(defstruct (record (:predicate first second)) value)",
        ),
        (
            "defstruct constructor NIL cannot have lambda list",
            r"(defstruct (record (:constructor nil (value))) value)",
        ),
        (
            "defstruct include requires an existing structure",
            r"(defstruct (record (:include missing)) value)",
        ),
        (
            "defstruct slot specification has too many elements",
            r"(defstruct record (value 0 nil extra))",
        ),
        (
            "defclass option must be a non-empty list",
            r"(defclass record () () 1)",
        ),
        (
            "defclass default initargs require pairs",
            r"(defclass record () () (:default-initargs :value))",
        ),
        (
            "defclass documentation requires a string",
            r"(defclass record () () (:documentation 1))",
        ),
        (
            "unsupported defclass option",
            r"(defclass record () () (:metaclass custom))",
        ),
        (
            "defclass slot options require values",
            r"(defclass record () ((value :initarg)))",
        ),
        (
            "defclass slot option must be supported",
            r"(defclass record () ((value :unknown t)))",
        ),
    ];

    for (name, source) in cases {
        assert!(
            Runtime::new().eval_source(source).is_err(),
            "{name}: {source}"
        );
    }
}

#[test]
fn evaluates_array_constructors_and_validation() {
    assert_eq!(
        evaluate(
            "(list (vector 1 2 3)
                   (svref (vector 4 5 6) 1)
                   (make-array 2 :initial-element 9)
                   (make-array 3 :initial-contents '(7 8 9)))",
        )
        .to_string(),
        "(#(1 2 3) 5 #(9 9) #(7 8 9))",
    );

    for source in [
        "(make-array)",
        "(make-array 2 :initial-element 0 :initial-contents '(1 2))",
        "(make-array 2 :unknown-option 0)",
        "(aref)",
        "(aref 1 0)",
        "(aref #(1 2))",
        "(aref #(1 2) 0 1)",
        "(aref #(1 2) 2)",
        "(svref)",
        "(svref 1 0)",
        "(svref '(1 2) 0)",
        "(svref #(1 2) 2)",
        "(row-major-aref)",
        "(row-major-aref #(1 2) 2)",
        "(array-row-major-index #(1 2))",
        "(array-in-bounds-p #(1 2))",
        "(array-dimension #(1 2) 1)",
        "(array-element-type)",
        "(array-element-type 1)",
        "(simple-array-p)",
        "(arrayp)",
        "(array-rank)",
        "(array-rank 1)",
        "(array-dimensions)",
        "(array-dimensions 1)",
        "(array-dimension)",
        "(array-dimension #(1 2))",
        "(array-dimension 1 0)",
        "(array-dimension #(1 2) -1)",
        "(array-total-size)",
        "(array-total-size 1)",
        "(bit)",
        "(bit 1 0)",
        "(bit #(2) 0)",
        "(bit #(0 1) 2)",
        "(aref #(1) -1)",
        "(aref #(1) 1.0)",
        "(aref #(1) 999999999999999999999999999999999999999999999999999999999999)",
        "(setf (aref #(1) 2) 3)",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn rejects_malformed_quasiquotes() {
    for source in ["(quasiquote)", "(quasiquote a b)", "`,@'(1 2)"] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_defstruct_boa_escaped_parameter_names() {
    let values = Runtime::new()
        .eval_source(
            r"(progn
                 (defstruct
                   (multi
                    (:constructor make-multi
                      (|REQA| &optional (|OPTB| 5 |OPTBP|)
                       &key ((:tag |KWNAME|) 9 |KWNAMEP|) bare-key)))
                   |REQA| |OPTB| |OPTBP| |KWNAME| |KWNAMEP| (bare-key 42))
                 (list
                   (write-to-string (make-multi 1))
                   (write-to-string (make-multi 1 2 :tag 7))
                   (not (ignore-errors (make-multi 1 'tag 9)))
                   (write-to-string (make-multi 1 :allow-other-keys t :unexpected-key 5))))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "(\"#S(MULTI :REQA 1 :OPTB 5 :OPTBP NIL :KWNAME 9 :KWNAMEP NIL :BARE-KEY 42)\" \
         \"#S(MULTI :REQA 1 :OPTB 2 :OPTBP NIL :KWNAME 7 :KWNAMEP NIL :BARE-KEY 42)\" T \
         \"#S(MULTI :REQA 1 :OPTB 5 :OPTBP NIL :KWNAME 9 :KWNAMEP NIL :BARE-KEY 42)\")"
    );
}

#[test]
fn evaluates_defstruct_boa_escaped_rest_and_auxiliary_parameters() {
    let values = Runtime::new()
        .eval_source(
            r"(progn
                 (defstruct
                   (boa2
                    (:constructor make-boa2
                      (first &rest |restZ| &aux (|auxA| (+ first 1)))))
                   first (extra 99) |restZ| |auxA|)
                 (write-to-string (make-boa2 1 2 3)))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        r##""#S(BOA2 :FIRST 1 :EXTRA 99 :RESTZ NIL :AUXA NIL)""##
    );
}

#[test]
fn evaluates_defstruct_boa_arity_errors_with_optional_parameters() {
    let cases = [
        r"(progn (defstruct (record (:constructor make-record (required &optional opt))) required opt) (make-record))",
        r"(progn (defstruct (record (:constructor make-record (required &optional opt))) required opt) (make-record 1 2 3))",
    ];
    for source in cases {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn rejects_invalid_slot_reader_and_writer_accessor_calls() {
    for source in [
        "(progn (defclass sa-point () ((x :accessor sa-point-x))) (sa-point-x))",
        "(progn (defclass sa-point () ((x :accessor sa-point-x))) (sa-point-x 1))",
        "(progn (defclass sa-point () ((x :accessor sa-point-x))) \
         (sa-point-x (make-instance 'sa-point)))",
        "(progn (defclass sa-point () ((x :writer set-sa-point-x))) (set-sa-point-x))",
        "(progn (defclass sa-point () ((x :writer set-sa-point-x))) (set-sa-point-x 1))",
        "(progn (defclass sa-point () ((x :writer set-sa-point-x))) (set-sa-point-x 1 2))",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn rejects_structure_predicate_accessor_and_copier_arity_errors() {
    for source in [
        "(progn (defstruct arity-person name) (arity-person-p))",
        "(progn (defstruct arity-person name) (arity-person-name))",
        "(progn (defstruct arity-person name) (copy-arity-person))",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_closures_with_escaped_pipe_quoted_parameter_names() {
    let values = Runtime::new()
        .eval_source(
            r"(progn
                 (defun mixed (|reqX| &optional (|optY| 5 |optYp|) &rest |restZ|
                               &aux (|auxA| (+ |reqX| 1)))
                   (list |reqX| |optY| |optYp| |restZ| |auxA|))
                 (list (mixed 1) (mixed 1 2 3 4)))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "((1 5 NIL NIL 2) (1 2 T (3 4) 2))");
}

#[test]
fn evaluates_closures_with_escaped_keyword_parameter_names() {
    let values = Runtime::new()
        .eval_source(
            r"(progn
                 (defun kw (&key ((:tag |kwName|) 9 |kwNameP|))
                   (list |kwName| |kwNameP|))
                 (list (kw) (kw :tag 3)))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "((9 NIL) (3 T))");
}

#[test]
fn rejects_closures_called_with_unpaired_keyword_arguments() {
    let result = Runtime::new().eval_source("(progn (defun kw (&key x) x) (kw :x))");
    assert!(result.is_err());
}

#[test]
fn signals_no_applicable_method_for_generic_function_calls() {
    let values = Runtime::new()
        .eval_source(
            r"(progn
                 (defclass gd-point () ())
                 (defgeneric gd (x))
                 (defmethod gd ((x gd-point)) x)
                 (let ((point (make-instance 'gd-point)))
                   (list (not (ignore-errors (gd)))
                         (not (ignore-errors (gd point 999))))))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(T T)");
}

#[test]
fn signals_no_primary_method_when_only_auxiliary_methods_are_defined() {
    let result = Runtime::new().eval_source(
        r"(progn
             (defclass np-point () ())
             (defgeneric np (x))
             (defmethod np :before ((x np-point)) x)
             (np (make-instance 'np-point)))",
    );
    assert!(result.is_err());
}

#[test]
fn evaluates_generic_function_with_multiple_around_methods_chaining_call_next_method() {
    let values = Runtime::new()
        .eval_source(
            r"(progn
                 (defclass mc-point () ())
                 (let ((events nil))
                   (defgeneric mc (x))
                   (defmethod mc ((x mc-point))
                     (setf events (cons :primary events))
                     :primary)
                   (defmethod mc :around ((x mc-point))
                     (setf events (cons :around-point events))
                     (call-next-method))
                   (defmethod mc :around ((x t))
                     (setf events (cons :around-t events))
                     (call-next-method))
                   (list (mc (make-instance 'mc-point)) (reverse events))))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "(:PRIMARY (:AROUND-POINT :AROUND-T :PRIMARY))"
    );
}

#[test]
fn evaluates_clos_class_direct_slots() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
             (defclass direct-slots-parent () ((inherited)))
             (defclass direct-slots-child (direct-slots-parent) ((own)))
             (class-direct-slots (find-class 'direct-slots-child)))"#,
        )
        .must_exist();
    assert_eq!(values[0].to_string(), "(OWN)");
}

#[test]
fn evaluates_clos_class_slots_including_inherited_slots() {
    let values = Runtime::new()
        .eval_source(
            r#"(progn
             (defclass effective-slots-parent () ((inherited)))
             (defclass effective-slots-child (effective-slots-parent) ((own)))
             (class-slots (find-class 'effective-slots-child)))"#,
        )
        .must_exist();
    assert_eq!(values[0].to_string(), "(OWN INHERITED)");
}

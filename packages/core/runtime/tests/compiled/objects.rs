#[test]
fn compiled_evaluates_builtin_method_combinations() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defgeneric all-true (x) (:method-combination and))
                 (defmethod all-true ((x t)) t)
                 (defmethod all-true ((x t)) nil)
                 (defgeneric any-true (x) (:method-combination or))
                 (defmethod any-true ((x t)) nil)
                 (defmethod any-true ((x t)) t)
                 (defgeneric sequence (x) (:method-combination progn))
                 (defmethod sequence ((x t)) 1)
                 (defmethod sequence ((x t)) 2)
                 (defgeneric collect-values (x) (:method-combination list))
                 (defmethod collect-values ((x t)) 1)
                 (defmethod collect-values ((x t)) 2)
                 (defgeneric append-values (x) (:method-combination append))
                 (defmethod append-values ((x t)) (list 1))
                 (defmethod append-values ((x t)) (list 2 3))
                 (defgeneric sum-values (x) (:method-combination +))
                 (defmethod sum-values ((x t)) 2)
                 (defmethod sum-values ((x t)) 3)
                 (defgeneric max-values (x) (:method-combination max))
                 (defmethod max-values ((x t)) 2)
                 (defmethod max-values ((x t)) 3)
                 (defgeneric min-values (x) (:method-combination min))
                 (defmethod min-values ((x t)) 2)
                 (defmethod min-values ((x t)) 3)
                 (defgeneric nconc-values (x) (:method-combination nconc))
                 (defmethod nconc-values ((x t)) (list 1))
                 (defmethod nconc-values ((x t)) (list 2 3))
                 (list (all-true 1) (any-true 1) (ncl-user::any-true 1)
                       (sequence 1) (collect-values 1) (append-values 1)
                       (sum-values 1) (max-values 1) (min-values 1)
                       (nconc-values 1)))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(NIL T T 2 (1 2) (1 2 3) 5 3 2 (1 2 3))");
}

#[test]
fn compiled_evaluates_native_make_instance_operation() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defclass native-make-instance-target ()
                   ((value :initarg :value)))
                 (slot-value
                   (make-instance 'native-make-instance-target :value 42)
                   'value))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "42");
}

#[test]
fn compiled_evaluates_basic_clos_instances_and_accessors() {
    let values = Runtime::new()
        .eval_compiled_source(
            r"(progn
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
                         (class-name (find-class 'point)))))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(2 2 3 T T T POINT POINT)");
}

#[test]
fn compiled_evaluates_clos_slot_value_setf() {
    let values = Runtime::new()
        .eval_compiled_source(
            r"(progn
                 (defclass slot-value-target () ((name :initarg :name)))
                 (let ((object (make-instance 'slot-value-target :name 1)))
                   (list (setf (slot-value object 'name) 2)
                         (slot-value object 'name))))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(2 2)");
}

#[test]
fn compiled_evaluates_clos_with_slots_and_accessors() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_compiled_source(
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
        .eval_compiled_source("(with-accessors (x) object x)")
        .is_err());
}

#[test]
fn compiled_evaluates_clos_slot_initialization_options() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_clos_class_allocated_slots() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_clos_default_initargs() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_clos_setf_and_generic_methods() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_clos_methods_with_ordinary_lambda_lists() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_clos_inheritance_and_specialization() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_clos_unbound_slots() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_enforces_clos_slot_types_on_initialization_and_writes() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_clos_method_combination() {
    let values = Runtime::new()
        .eval_compiled_source(
            r"(progn
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
                     (list (point-value point) (reverse events)))))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "((:AROUND (:PRIMARY T (:BASE NIL))) (:AROUND-BEFORE :BEFORE :PRIMARY :BASE :AFTER :AROUND-AFTER))"
    );
}

#[test]
fn compiled_evaluates_defstruct_constructors_accessors_and_copies() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_defstruct_name_and_options() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_defstruct_read_only_slots() {
    let values = Runtime::new()
        .eval_compiled_source(
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
        .eval_compiled_source(
            r"(progn
                 (defstruct immutable (id 0 t))
                 (let ((record (make-immutable :id 1)))
                   (setf (immutable-id record) 2)))",
        )
        .must_fail();
    assert!(matches!(
        error,
        RuntimeError::InvalidForm { message, .. }
            if message == "cannot SETF a read-only structure slot"
    ));
}

#[test]
fn compiled_evaluates_defstruct_included_slots_and_type_hierarchy() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_defstruct_boa_constructors() {
    let values = Runtime::new()
        .eval_compiled_source(
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
fn compiled_evaluates_array_constructors_and_validation() {
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
        "(make-array 2 :initial-element 0 :initial-contents '(1 2))",
        "(make-array 2 :unknown-option 0)",
        "(svref '(1 2) 0)",
        "(row-major-aref #(1 2) 2)",
        "(array-dimension #(1 2) 1)",
    ] {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn compiled_evaluates_native_array_accessors() {
    assert_eq!(
        evaluate(
            "(list (aref (make-array '(2 2) :initial-contents '((1 2) (3 4))) 1 0)
                   (svref (vector 4 5 6) 1)
                   (bit #(0 1) 1)
                   (row-major-aref #(7 8 9) 2))",
        )
        .to_string(),
        "(3 5 1 9)",
    );
}

#[test]
fn compiled_evaluates_adjust_array() {
    let values = Runtime::new()
        .eval_compiled_source(
            r"(let* ((array (make-array 2 :initial-contents '(4 5)))
                      (adjusted (adjust-array array 4)))
                 (list (array-dimensions adjusted)
                       (aref adjusted 0) (aref adjusted 1)
                       (aref adjusted 2) (aref adjusted 3)))",
        )
        .must_exist();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "((4) 4 5 NIL NIL)");
}

#[test]
fn compiled_evaluates_native_array_metadata() {
    assert_eq!(
        evaluate(
            "(list (array-element-type #(1 2))
                   (array-element-type (make-array 2 :element-type 'character))
                   (array-rank (make-array '(2 3)))
                   (array-dimensions (make-array '(2 3)))
                   (array-dimension (make-array '(2 3)) 1)
                   (array-total-size (make-array '(2 3))))",
        )
        .to_string(),
        "(T CHARACTER 2 (2 3) 3 6)",
    );
}

#[test]
fn compiled_evaluates_native_subseq() {
    assert_eq!(
        evaluate("(list (subseq '(a b c d) 1 3) (subseq \"abcd\" 0 2))").to_string(),
        "((B C) \"ab\")",
    );
}

#[test]
fn compiled_evaluates_native_copy_seq() {
    assert_eq!(evaluate("(copy-seq \"abc\")").to_string(), "\"abc\"");
}

#[test]
fn compiled_evaluates_native_sequence_mutations() {
    assert_eq!(
        evaluate("(list (fill 9 (vector 1 2 3) :start 1 :end 3) (replace (vector 0 0 0) #(4 5) :start1 1))").to_string(),
        "(#(1 9 9) #(0 4 5))",
    );
}

#[test]
fn compiled_evaluates_native_concatenate() {
    assert_eq!(
        evaluate("(list (concatenate 'list '(a b) #(c d)) (concatenate 'string \"ab\" \"cd\"))",)
            .to_string(),
        "((A B C D) \"abcd\")",
    );
}

#[test]
fn compiled_evaluates_native_sequence_conversions() {
    assert_eq!(
        evaluate(
            "(list (make-sequence 'list 2 :initial-element 7)
                   (coerce '(1 2) 'vector))",
        )
        .to_string(),
        "((7 7) #(1 2))",
    );
}

#[test]
fn compiled_evaluates_native_string_case() {
    assert_eq!(
        evaluate("(list (string-upcase \"ab c\") (nstring-downcase \"AB C\" :start 1))")
            .to_string(),
        "(\"AB C\" \"Ab c\")",
    );
}

#[test]
fn compiled_evaluates_native_string_comparisons() {
    assert_eq!(
        evaluate("(list (string= \"a\" \"a\") (string-equal \"A\" \"a\") (string< \"a\" \"b\") (string> \"b\" \"a\") (string<= \"a\" \"a\") (string>= \"b\" \"a\"))").to_string(),
        "(T T 0 0 1 0)",
    );
}

#[test]
fn compiled_evaluates_native_character_comparisons() {
    assert_eq!(
        evaluate("(list (char= #\\a #\\a) (char/= #\\a #\\b #\\c) (char-equal #\\A #\\a) (char< #\\a #\\b) (char-not-greaterp #\\A #\\a))").to_string(),
        "(T T T T T)",
    );
}

#[test]
fn compiled_rejects_invalid_hash_table_options() {
    for source in [
        "(make-hash-table :test #'not-a-hash-test)",
        "(make-hash-table :size -1)",
        "(make-hash-table :rehash-size 0)",
        "(make-hash-table :rehash-threshold 2)",
        "(make-hash-table :unknown-option t)",
    ] {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn compiled_evaluates_native_string_trimming() {
    assert_eq!(
        evaluate(
            "(list (string-trim \" \" \" hi \") (string-left-trim \" \" \" hi \") (string-right-trim \" \" \" hi \"))"
        )
        .to_string(),
        "(\"hi\" \"hi \" \" hi\")",
    );
}

#[test]
fn compiled_evaluates_native_string_construction() {
    assert_eq!(
        evaluate("(list (string 'foo) (make-string 2) (make-string 3 #\\x))").to_string(),
        "(\"FOO\" \"  \" \"xxx\")",
    );
}

#[test]
fn compiled_evaluates_native_character_case_operations() {
    assert_eq!(
        evaluate("(list (char-upcase #\\a) (char-downcase #\\Z))").to_string(),
        "(#\\A #\\z)",
    );
}

#[test]
fn compiled_evaluates_native_character_name_operations() {
    assert_eq!(
        evaluate("(list (char-name #\\Newline) (name-char \"space\"))").to_string(),
        "(\"Newline\" #\\SPACE)",
    );
}

#[test]
fn compiled_evaluates_native_digit_character_predicate() {
    assert_eq!(
        evaluate("(list (digit-char-p #\\5) (digit-char-p #\\G))").to_string(),
        "(5 NIL)"
    );
}

#[test]
fn compiled_rejects_invalid_defstruct_invocations() {
    let cases = [
        (
            "odd constructor arguments",
            "(progn (defstruct record id) (make-record :id))",
        ),
        (
            "non-keyword constructor name",
            "(progn (defstruct record id) (make-record 1 2))",
        ),
        (
            "unknown constructor keyword",
            "(progn (defstruct record id) (make-record :missing 2))",
        ),
        (
            "accessor receives the wrong type",
            "(progn (defstruct record id) (record-id 1))",
        ),
        (
            "copier receives the wrong type",
            "(progn (defstruct (record (:copier copy-record)) id) (copy-record 1))",
        ),
    ];

    for (name, source) in cases {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "expected {name} to fail: {source}"
        );
    }
}
use super::*;

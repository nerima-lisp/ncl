use super::{Runtime, RuntimeError, evaluate};

#[test]
fn compiled_evaluates_basic_clos_instances_and_accessors() {
    let values = Runtime::new()
        .eval_compiled_source(
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
                         (class-name (find-class 'point))
                         (find-class 'missing nil)
                         (class-name (find-class 'point t nil)))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(2 2 3 T T T POINT POINT NIL POINT)");
}

#[test]
fn compiled_evaluates_clos_with_slots_and_accessors() {
    let runtime = Runtime::new();
    let values = runtime
        .eval_compiled_source(
            r#"(progn
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
                             (ws-point-y point))))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(5 7 5 7 11 11 7)");

    assert!(
        runtime
            .eval_compiled_source("(with-accessors (x) object x)")
            .is_err()
    );
}

#[test]
fn compiled_evaluates_clos_slot_initialization_options() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defclass defaults ()
                   ((x :initform 7 :reader defaults-x)
                    (y :initarg :y :writer set-defaults-y)
                    (z :initarg nil)))
                 (let ((object (make-instance 'defaults :y 3 :ignored 5 :allow-other-keys t)))
                   (set-defaults-y 9 object)
                   (list (defaults-x object)
                         (slot-value object 'y)
                         (slot-boundp object 'z)
                         (not (ignore-errors (make-instance 'defaults :x 1)))
                         (not (ignore-errors (make-instance 'defaults :x 1 :allow-other-keys nil))))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(7 9 NIL T T)");
}

#[test]
fn compiled_evaluates_clos_class_allocated_slots() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
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
                           (slot-boundp child 'value)))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "((7 7 T T) NIL NIL)");
}

#[test]
fn compiled_rejects_unsupported_defclass_slot_allocation() {
    let error = Runtime::default()
        .eval_compiled_source(
            r#"(progn
                 (defclass invalid-allocation-point-compiled ()
                   ((x :allocation :bogus)))
                 t)"#,
        )
        .expect_err("unsupported defclass slot allocation should fail");

    assert!(
        error
            .to_string()
            .contains("unsupported defclass allocation")
    );
}

#[test]
fn compiled_rejects_duplicate_defclass_slot_names() {
    let error = Runtime::default()
        .eval_compiled_source(
            r#"(progn
                 (defclass duplicate-slot-point-compiled ()
                   ((x :initarg :x :initform 1)
                    (x :initarg :y :initform 2)))
                 t)"#,
        )
        .expect_err("duplicate defclass slot names should fail");

    assert!(error.to_string().contains("duplicate defclass slot name"));
}

#[test]
fn compiled_rejects_duplicate_defclass_superclasses() {
    let error = Runtime::default()
        .eval_compiled_source(
            r#"(progn
                 (defclass duplicate-superclass-point-compiled (standard-object standard-object) ())
                 t)"#,
        )
        .expect_err("duplicate defclass superclasses should fail");

    assert!(error.to_string().contains("duplicate defclass superclass"));
}

#[test]
fn compiled_evaluates_clos_default_initargs() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defclass defaults ()
                   ((value :initarg :value :initform 1))
                   (:default-initargs :value (+ 2 5))
                   (:documentation "defaulted class"))
                 (defclass child-defaults (defaults) ())
                 (let ((explicit (make-instance 'child-defaults :value 9))
                       (implicit (make-instance 'child-defaults)))
                   (list (slot-value explicit 'value)
                         (slot-value implicit 'value)
                         (documentation (find-class 'defaults) t))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), r#"(9 7 "defaulted class")"#);
}

#[test]
fn compiled_evaluates_defclass_standard_class_metaclass_option() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defclass metaclass-point ()
                   ((x :initarg :x))
                   (:metaclass standard-class))
                 (let ((object (make-instance 'metaclass-point :x 5)))
                   (list (class-name (class-of object))
                         (class-name (find-class 'metaclass-point))
                         (slot-value object 'x))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(METACLASS-POINT METACLASS-POINT 5)");
}

#[test]
fn compiled_rejects_unsupported_defclass_metaclass_option() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(not
                (ignore-errors
                  (defclass unsupported-metaclass-point ()
                    ()
                    (:metaclass funcallable-standard-class))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "T");
}

#[test]
fn compiled_rejects_unknown_defclass_option() {
    let runtime = Runtime::default();
    let values = runtime
        .eval_compiled_source(
            r#"(not
                (ignore-errors
                  (defclass unknown-option-point-compiled () ()
                    (:unknown-option t))))"#,
        )
        .expect("compiled evaluation should succeed");

    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "T");
}

#[test]
fn compiled_evaluates_function_documentation() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defun documented-function (value)
                   "function doc"
                   value)
                 (defgeneric documented-generic (object)
                   (:documentation "generic doc")
                   (:method (object) object))
                 (list (documentation 'documented-function 'function)
                       (documentation 'documented-generic 'function)
                       (documentation 'missing-documentation 'function)
                       (documented-function 7)
                       (documented-generic 9)))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        r#"("function doc" "generic doc" NIL 7 9)"#
    );
}

#[test]
fn compiled_evaluates_variable_documentation() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defvar *compiled-documented-variable* 1 "variable doc")
                 (defvar *compiled-documented-variable* 2 "updated variable doc")
                 (defparameter *compiled-documented-parameter* 3 "parameter doc")
                 (list *compiled-documented-variable*
                       (documentation '*compiled-documented-variable* 'variable)
                       (documentation '*compiled-documented-parameter* 'variable)
                       (documentation '*compiled-missing-variable-documentation* 'variable)))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        r#"(1 "updated variable doc" "parameter doc" NIL)"#
    );
}

#[test]
fn compiled_evaluates_setf_function_documentation() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defun setf-documented-function (value)
                   value)
                 (defgeneric setf-documented-generic (object)
                   (:method (object) object))
                 (list (setf (documentation 'setf-documented-function 'function) "function doc")
                       (documentation 'setf-documented-function 'function)
                       (setf (documentation 'setf-documented-function 'function) nil)
                       (documentation 'setf-documented-function 'function)
                       (setf (documentation 'setf-documented-generic 'function) "generic doc")
                       (documentation 'setf-documented-generic 'function)))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        r#"("function doc" "function doc" NIL NIL "generic doc" "generic doc")"#
    );
}

#[test]
fn compiled_evaluates_setf_variable_documentation() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defvar *compiled-setf-documented-variable* 1)
                 (list (setf (documentation '*compiled-setf-documented-variable* 'variable) "variable doc")
                       (documentation '*compiled-setf-documented-variable* 'variable)
                       (setf (documentation '*compiled-setf-documented-variable* 'variable) nil)
                       (documentation '*compiled-setf-documented-variable* 'variable)))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        r#"("variable doc" "variable doc" NIL NIL)"#
    );
}

#[test]
fn compiled_evaluates_setf_class_and_package_documentation() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defclass setf-doc-class-compiled () ())
                 (defpackage :setf-doc-package-compiled)
                 (let ((class (find-class 'setf-doc-class-compiled))
                       (package (find-package :setf-doc-package-compiled)))
                   (list (setf (documentation class t) "class doc")
                         (documentation class t)
                         (setf (documentation class t) nil)
                         (documentation class t)
                         (setf (documentation package t) "package doc")
                         (documentation package t)
                         (setf (documentation package t) nil)
                         (documentation package t))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        r#"("class doc" "class doc" NIL NIL "package doc" "package doc" NIL NIL)"#
    );
}

#[test]
fn compiled_evaluates_clos_defgeneric_method_options() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defclass generic-option-point ()
                   ((x :initarg :x :accessor generic-option-point-x)))
                 (defgeneric generic-option-value (object)
                   (:method ((object generic-option-point))
                     (generic-option-point-x object))
                   (:method :before ((object generic-option-point))
                     (setf (generic-option-point-x object)
                           (+ (generic-option-point-x object) 1))))
                 (let ((point (make-instance 'generic-option-point :x 4)))
                   (list (generic-option-value point)
                         (generic-option-point-x point))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(5 5)");
}

#[test]
fn compiled_evaluates_clos_ensure_generic_function() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (ensure-generic-function 'ensured-value :lambda-list '(object))
                 (ensure-generic-function 'ensured-extra
                                          :lambda-list '(object)
                                          :unknown-option 1
                                          :allow-other-keys t)
                 (defun ensured-conflict (object) object)
                 (defmethod ensured-value ((object t))
                   (list :value object))
                 (let ((same (ensure-generic-function 'ensured-value
                              :lambda-list '(object))))
                   (list (functionp same)
                         (ensured-value 7)
                         (fboundp 'ensured-value)
                         (functionp (symbol-function 'ensured-extra))
                         (handler-case
                             (ensure-generic-function 'ensured-conflict
                                                      :lambda-list '(object))
                           (error () :error)))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(T (:VALUE 7) T T :ERROR)");
}

#[test]
fn compiled_evaluates_clos_find_method() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defclass find-method-point () ())
                 (defgeneric find-method-value (object))
                 (defmethod find-method-value ((object find-method-point)) :primary)
                 (defmethod find-method-value :before ((object find-method-point)) nil)
                 (let ((class (find-class 'find-method-point)))
                   (list (typep (find-method #'find-method-value '() (list class))
                                'method)
                         (typep (find-method #'find-method-value '(:before) (list class))
                                'method)
                         (eq (find-method #'find-method-value '() (list class))
                             (find-method #'find-method-value '() (list class)))
                         (find-method #'find-method-value '(:after) (list class) nil)
                         (handler-case
                             (find-method #'find-method-value '(:after) (list class))
                           (error () :error)))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(T T T NIL :ERROR)");
}

#[test]
fn compiled_evaluates_clos_method_accessors() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defclass method-accessor-point () ())
                 (defgeneric method-accessor-value (object tag))
                 (defmethod method-accessor-value :around
                     ((object method-accessor-point) (tag (eql :tag)))
                   :around)
                 (let* ((class (find-class 'method-accessor-point))
                        (method (find-method #'method-accessor-value
                                             '(:around)
                                             (list class '(eql :tag))))
                        (specializers (method-specializers method)))
                   (list (method-qualifiers method)
                         (class-name (car specializers))
                         (car (cdr specializers))
                         (handler-case
                             (method-qualifiers class)
                           (error () :error)))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "((:AROUND) METHOD-ACCESSOR-POINT (EQL :TAG) :ERROR)"
    );
}

#[test]
fn compiled_evaluates_clos_compute_applicable_methods() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defclass applicable-parent () ())
                 (defclass applicable-child (applicable-parent) ())
                 (defgeneric applicable-value (object tag))
                 (defmethod applicable-value ((object t) (tag (eql :hit))) :eql)
                 (defmethod applicable-value ((object applicable-parent) tag) :parent)
                 (defmethod applicable-value ((object applicable-child) tag) :child)
                 (let* ((object (make-instance 'applicable-child))
                        (methods (compute-applicable-methods #'applicable-value
                                                             (list object :hit))))
                   (list (mapcar (lambda (method)
                                   (class-name (car (method-specializers method))))
                                 methods)
                         (typep (car methods) 'method)
                         (compute-applicable-methods #'applicable-value (list 42 :miss))
                         (handler-case
                             (compute-applicable-methods #'+ (list 1 2))
                           (error () :error))
                         (handler-case
                             (compute-applicable-methods #'applicable-value object)
                           (error () :error)))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "((APPLICABLE-CHILD APPLICABLE-PARENT T) T NIL :ERROR :ERROR)"
    );
}

#[test]
fn compiled_evaluates_clos_generic_function_accessors() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defclass generic-accessor-parent () ())
                 (defclass generic-accessor-child (generic-accessor-parent) ())
                 (defgeneric generic-accessor-value (object))
                 (defmethod generic-accessor-value ((object generic-accessor-parent)) :parent)
                 (defmethod generic-accessor-value ((object generic-accessor-child)) :child)
                 (let* ((function #'generic-accessor-value)
                        (methods (generic-function-methods function)))
                   (list (generic-function-name function)
                         (length methods)
                         (mapcar (lambda (method)
                                   (class-name (car (method-specializers method))))
                                 methods)
                         (method-lambda-list (car methods))
                         (eq function (method-generic-function (car methods)))
                         (functionp (method-function (car methods)))
                         (typep (car methods) 'method)
                         (typep function 'generic-function)
                         (typep function 'standard-generic-function)
                         (typep (car methods) 'standard-method)
                         (class-name (generic-function-class function))
                         (class-name (method-class (car methods)))
                         (method-combination function)
                         (handler-case
                             (generic-function-methods #'+)
                           (error () :error))
                         (handler-case
                             (generic-function-name #'+)
                           (error () :error))
                         (handler-case
                             (generic-function-class #'+)
                           (error () :error))
                         (handler-case
                             (method-lambda-list function)
                           (error () :error))
                         (handler-case
                             (method-class function)
                           (error () :error))
                         (handler-case
                             (method-generic-function function)
                           (error () :error))
                         (handler-case
                             (method-function function)
                           (error () :error))
                         (handler-case
                             (method-combination #'+)
                           (error () :error)))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "(GENERIC-ACCESSOR-VALUE 2 (GENERIC-ACCESSOR-PARENT GENERIC-ACCESSOR-CHILD) (OBJECT) T T T T T T STANDARD-GENERIC-FUNCTION STANDARD-METHOD STANDARD :ERROR :ERROR :ERROR :ERROR :ERROR :ERROR :ERROR :ERROR)"
    );
}

#[test]
fn compiled_evaluates_clos_setf_and_generic_methods() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
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
                         (point-total point))))"#,
        )
        .unwrap();
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
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "((\"default\" \"suffix\" NIL NIL) (\"given\" \"tail\" T T) (1 2 3))"
    );
}

#[test]
fn compiled_rejects_non_congruent_clos_method_lambda_lists() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(list
                 (not
                   (ignore-errors
                     (progn
                       (defclass point () ())
                       (defgeneric describe-point (object &optional prefix))
                       (defmethod describe-point ((object point))
                         object))))
                 (not
                   (ignore-errors
                     (progn
                       (defclass point-with-key () ())
                       (defgeneric point-suffix (object &key suffix))
                       (defmethod point-suffix ((object point-with-key) &key tail)
                         tail))))
                 (not
                   (ignore-errors
                     (progn
                       (defclass point-with-any-key () ())
                       (defgeneric point-any-key (object &key suffix &allow-other-keys))
                       (defmethod point-any-key ((object point-with-any-key) &key suffix)
                         suffix)))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(T T T)");
}

#[test]
fn compiled_evaluates_clos_inheritance_and_specialization() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
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
                         (point-coordinate point))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(4 :RED T T (4))");
}

#[test]
fn compiled_evaluates_clos_c3_precedence_and_leftmost_method_order() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defclass root () ())
                 (defclass left (root) ())
                 (defclass right (root) ())
                 (defclass diamond (left right) ())
                 (defgeneric describe-diamond (object))
                 (defmethod describe-diamond ((object root)) :root)
                 (defmethod describe-diamond ((object right)) :right)
                 (defgeneric choose-pair (first second))
                 (defmethod choose-pair ((first right) (second left)) :right-left)
                 (defmethod choose-pair ((first left) (second right)) :left-right)
                 (let ((object (make-instance 'diamond)))
                   (list (describe-diamond object)
                         (choose-pair object object))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(:RIGHT :LEFT-RIGHT)");
}

#[test]
fn compiled_evaluates_clos_eql_specializer() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defgeneric choose-number (value))
                 (defmethod choose-number ((value t)) :default)
                 (defmethod choose-number ((value (eql 7))) :seven)
                 (list (choose-number 7) (choose-number 8)))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(:SEVEN :DEFAULT)");
}

#[test]
fn compiled_evaluates_clos_unbound_slots() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
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
                       (slot-boundp point 'x)))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(T NIL T (T 9) NIL)");
}

#[test]
fn compiled_evaluates_clos_method_combination() {
    let values = Runtime::new()
        .eval_compiled_source(
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
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "((:AROUND (:PRIMARY T (:BASE NIL))) (:AROUND-BEFORE :BEFORE :PRIMARY :BASE :AFTER :AROUND-AFTER))"
    );
}

#[test]
fn compiled_rejects_call_next_method_arguments_that_change_ordered_applicable_methods() {
    let error = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defclass point () ())
                 (defgeneric point-value (object))
                 (defmethod point-value ((object t))
                   :base)
                 (defmethod point-value ((object point))
                   (call-next-method 42))
                 (point-value (make-instance 'point)))"#,
        )
        .unwrap_err();
    assert!(matches!(
        error,
        RuntimeError::InvalidForm { message, .. }
            if message
                == "call-next-method arguments changed the ordered applicable methods for POINT-VALUE"
    ));
}

#[test]
fn compiled_evaluates_clos_no_applicable_method_hook() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defgeneric no-applicable-method (generic-function &rest arguments))
                 (defmethod no-applicable-method ((generic-function t) &rest arguments)
                   (list (functionp generic-function) arguments))
                 (defgeneric point-value (object))
                 (point-value 42))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(T (42))");
}

#[test]
fn compiled_evaluates_clos_no_next_method_hook() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defgeneric no-next-method (generic-function method &rest arguments))
                 (defmethod no-next-method ((generic-function t) (method t) &rest arguments)
                   (list
                     (functionp generic-function)
                     (functionp method)
                     (typep (car arguments) 'point)))
                 (defclass point () ())
                 (defgeneric point-value (object))
                 (defmethod point-value ((object point))
                   (call-next-method))
                 (point-value (make-instance 'point)))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(T T T)");
}

#[test]
fn compiled_evaluates_clos_defmethod_redefinition_replaces_existing_method() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defclass point () ())
                 (defgeneric point-value (object))
                 (defmethod point-value ((object point)) :first)
                 (defmethod point-value ((object point)) :second)
                 (point-value (make-instance 'point)))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), ":SECOND");
}

#[test]
fn compiled_evaluates_clos_initialize_instance_after_method_without_primary() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defclass point ()
                   ((x :initarg :x :accessor point-x)))
                 (defmethod initialize-instance :after ((object point) &key x)
                   (setf (slot-value object 'x) (+ x 1)))
                 (point-x (make-instance 'point :x 2)))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "3");
}

#[test]
fn compiled_evaluates_clos_initialize_instance_before_method_runs_before_standard_initialization() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defparameter *seen* nil)
                 (defclass point ()
                   ((x :initarg :x :accessor point-x)))
                 (defmethod initialize-instance :before ((object point) &key x)
                   (setf *seen* (slot-boundp object 'x)))
                 (let ((point (make-instance 'point :x 10)))
                   (list *seen* (point-x point))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(NIL 10)");
}

#[test]
fn compiled_evaluates_clos_shared_initialize_reinitializes_requested_slots() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defclass point ()
                   ((x :initarg :x :accessor point-x :initform 1)
                    (y :initarg :y :accessor point-y :initform 2)))
                 (let ((point (make-instance 'point :x 10 :y 20)))
                   (slot-makunbound point 'x)
                   (slot-makunbound point 'y)
                   (shared-initialize point '(x) :y 30)
                   (list (slot-boundp point 'x)
                         (point-x point)
                         (point-y point))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(T 1 30)");
}

#[test]
fn compiled_evaluates_clos_reinitialize_instance_updates_initargs_and_runs_methods() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defclass point ()
                   ((x :initarg :x :accessor point-x :initform 1)
                    (y :initarg :y :accessor point-y :initform 2)))
                 (defmethod reinitialize-instance ((object point) &key y)
                   (prog1 (call-next-method)
                     (when y
                       (setf (slot-value object 'x) (+ (point-y object) 1)))))
                 (let ((point (make-instance 'point :x 10 :y 20)))
                   (slot-makunbound point 'x)
                   (slot-makunbound point 'y)
                   (reinitialize-instance point :y 30 :ignored 4 :allow-other-keys t)
                   (list (slot-boundp point 'x)
                         (slot-boundp point 'y)
                         (point-x point)
                         (point-y point))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(T T 31 30)");
}

#[test]
fn compiled_evaluates_clos_reinitialize_instance_before_method_runs_before_standard_reinitialization()
 {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defparameter *seen* nil)
                 (defclass point ()
                   ((x :initarg :x :accessor point-x :initform 1)
                    (y :initarg :y :accessor point-y :initform 2)))
                 (defmethod reinitialize-instance :before ((object point) &key y)
                   (setf *seen* (list (slot-boundp object 'x)
                                      (slot-boundp object 'y)
                                      (when (slot-boundp object 'y)
                                        (point-y object)))))
                 (let ((point (make-instance 'point :x 10 :y 20)))
                   (slot-makunbound point 'x)
                   (slot-makunbound point 'y)
                   (reinitialize-instance point :y 30)
                   (list *seen*
                         (slot-boundp point 'y)
                         (point-y point))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "((NIL NIL NIL) T 30)");
}

#[test]
fn compiled_evaluates_clos_change_class_preserves_shared_slots_and_reinitializes_new_ones() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defclass point ()
                   ((x :initarg :x :accessor point-x)
                    (y :initarg :y :accessor point-y :initform 2)))
                 (defclass colored-point (point)
                   ((color :initarg :color :accessor point-color :initform :blue)))
                 (let ((point (make-instance 'point :x 10 :y 20)))
                   (list
                     (eq point (change-class point 'colored-point :color :red :ignored 9 :allow-other-keys t))
                     (typep point 'colored-point)
                     (point-x point)
                     (point-y point)
                     (point-color point)
                     (ignore-errors (slot-value point 'missing)))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(T T 10 20 :RED NIL)");
}

#[test]
fn compiled_evaluates_clos_change_class_invokes_update_instance_for_different_class() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defclass point ()
                   ((x :initarg :x :accessor point-x)
                    (y :initarg :y :accessor point-y)))
                 (defclass colored-point (point)
                   ((color :initarg :color :accessor point-color :initform :blue)
                    (old-x :accessor point-old-x)))
                 (defmethod update-instance-for-different-class ((previous point) (current colored-point) &key color)
                   (when color
                     (setf (point-old-x current) (point-x previous)))
                   current)
                 (let ((point (make-instance 'point :x 10 :y 20)))
                   (list
                     (eq point (change-class point 'colored-point :color :red))
                     (point-x point)
                     (point-y point)
                     (point-color point)
                     (point-old-x point))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(T 10 20 :RED 10)");
}

#[test]
fn compiled_evaluates_clos_slot_missing_and_slot_unbound_hooks() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defparameter *compiled-slot-events* nil)
                 (defclass point ()
                   ((x :accessor point-x)
                    (y :accessor point-y :initarg :y)))
                 (defmethod slot-missing ((class t) object slot-name operation &optional new-value)
                   (push (list (class-name class) (typep object 'point) slot-name operation new-value)
                         *compiled-slot-events*)
                   :missing)
                 (defmethod slot-unbound ((class t) object slot-name)
                   (list (class-name class) (typep object 'point) slot-name))
                 (let ((point (make-instance 'point :y 2)))
                   (list
                     (slot-value point 'missing)
                     (slot-boundp point 'missing)
                     (eq point (slot-makunbound point 'missing))
                     (setf (slot-value point 'missing) 7)
                     (slot-value point 'x)
                     (point-x point)
                     (reverse *compiled-slot-events*))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "(:MISSING :MISSING T 7 (POINT T X) (POINT T X) ((POINT T MISSING SLOT-VALUE NIL) (POINT T MISSING SLOT-BOUNDP NIL) (POINT T MISSING SLOT-MAKUNBOUND NIL) (POINT T MISSING SETF 7)))"
    );
}

#[test]
fn compiled_evaluates_clos_allocate_instance_returns_uninitialized_standard_object() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defclass point ()
                   ((x :initarg :x :accessor point-x :initform 1)
                    (y :initarg :y :accessor point-y)))
                 (let ((point (allocate-instance 'point)))
                   (list (typep point 'point)
                         (slot-boundp point 'x)
                         (slot-boundp point 'y)
                         (ignore-errors (slot-value point 'x))
                         (ignore-errors (slot-value point 'y)))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(T NIL NIL NIL NIL)");
}

#[test]
fn compiled_evaluates_the_with_type_designators() {
    assert_eq!(
        evaluate(
            "(list (the integer (+ 3 4))
                    (the rational 1/2)
                    (the float 0.5)
                    (ignore-errors (the integer 1/2)))",
        )
        .to_string(),
        "(7 1/2 0.5 NIL)"
    );
}

#[test]
fn compiled_evaluates_locally_and_eval_when() {
    assert_eq!(
        evaluate(
            "(let ((seen 0))
               (list
                 (locally
                   (declare (type integer seen))
                   (setq seen 4)
                   seen)
                 (eval-when (:execute) (+ seen 1))
                 (eval-when (:compile-toplevel) (setq seen 99))
                 (progn
                   (declaim (optimize speed))
                   (proclaim '(inline seen))
                   seen)))",
        )
        .to_string(),
        "(4 5 NIL 4)"
    );
}

#[test]
fn compiled_evaluates_with_compilation_unit() {
    assert_eq!(
        evaluate(
            "(let ((seen 0))
               (list
                 (special-operator-p 'with-compilation-unit)
                 (with-compilation-unit ()
                   (setq seen (+ seen 2))
                   seen)
                 seen
                 (with-compilation-unit ()
                   (setq seen (+ seen 3))
                   seen)))",
        )
        .to_string(),
        "(T 2 2 5)"
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
                         (class-name (class-of person))
                         (class-name (find-class 'person))
                         (eq person (copy-person person))
                         (equal person (copy-person person))
                         (write-to-string person))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        r##"(T "Grace" 21 T PERSON PERSON PERSON NIL T "#S(PERSON :NAME \"Grace\" :AGE 21)")"##,
    );
}

#[test]
fn compiled_evaluates_structure_literal_dispatch() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defstruct person name (age 21))
                 (let ((person #S(person :name "Ada" :age 42)))
                   (list (person-p person)
                         (person-name person)
                         (person-age person)
                         (write-to-string person))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        r##"(T "Ada" 42 "#S(PERSON :NAME \"Ada\" :AGE 42)")"##,
    );
}

#[test]
fn compiled_evaluates_pathname_literal_dispatch() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(list (stringp #P"/tmp/demo.txt")
                     (equal #P"/tmp/demo.txt" "/tmp/demo.txt"))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(T T)");
}

#[test]
fn compiled_evaluates_defstruct_name_and_options() {
    let values = Runtime::new()
        .eval_compiled_source(
            r#"(progn
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
                 (defstruct
                   (named-record
                    (:named)
                    (:constructor make-named-record))
                   value)
                 (let ((account (make-account-record :id 7))
                       (plain (make-plain :amount 9))
                       (named (make-named-record :value 12)))
                   (list (account-p account)
                         (acct-id account)
                         (acct-balance account)
                         (equal account (clone-account account))
                         (typep account 'account)
                         (type-of account)
                         (amount plain)
                         (plain-p plain)
                         (equal plain (clone-plain plain))
                         (named-record-p named)
                         (typep named 'named-record)
                         (type-of named)
                         (write-to-string named))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        r##"(T 7 0 T T ACCOUNT 9 T T T T NAMED-RECORD "#S(NAMED-RECORD :VALUE 12)")"##
    );
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
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), r#"(7 "after")"#);

    let error = Runtime::new()
        .eval_compiled_source(
            r#"(progn
                 (defstruct immutable (id 0 t))
                 (let ((record (make-immutable :id 1)))
                   (setf (immutable-id record) 2)))"#,
        )
        .unwrap_err();
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
        .unwrap();
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
            r#"(progn
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
                           (boa-sum explicit)))))"#,
        )
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].to_string(),
        "((1 20 21 NIL T 21) (1 2 3 (:FLAG NIL) NIL 3))",
    );
}

#[test]
fn compiled_evaluates_arrays_and_multidimensional_setf() {
    assert_eq!(
        evaluate(
            "(let ((array (make-array '(2 2) :initial-element 0))
                   (vector (make-array 3 :initial-element 5)))
               (setf (aref array 1 0) 7
                     (aref vector 2) 9)
               (list (arrayp array) (array-rank array) (array-dimensions array)
                     (array-dimension array 1) (array-total-size array)
                     (aref array 1 0) (row-major-aref array 2)
                     (aref vector 2) (typep array 'array)))",
        )
        .to_string(),
        "(T 2 (2 2) 2 4 7 7 9 T)"
    );
    assert_eq!(
        evaluate(
            "(let ((array (make-array '(2 2)
                                      :initial-contents '((1 2) (3 4)))))
               (list (aref array 0 1) (aref array 1 0)
                     (row-major-aref array 3)))",
        )
        .to_string(),
        "(2 3 4)"
    );
    assert_eq!(
        evaluate(
            "(let ((array (make-array '(2 3)
                                      :element-type 'character
                                      :initial-contents '((0 1 2) (3 4 5)))))
               (list (array-row-major-index array 1 2)
                     (array-in-bounds-p array 1 2)
                     (array-in-bounds-p array 2 0)
                     (aref array 1 2)
                     (row-major-aref array (array-row-major-index array 1 2))
                     (array-element-type array)
                     (simple-array-p array)
                     (simple-vector-p (vector 1 2))
                     (simple-vector-p array)))",
        )
        .to_string(),
        "(5 T NIL 5 5 CHARACTER T T NIL)"
    );
    assert_eq!(
        evaluate(
            "(list (upgraded-array-element-type 'bit)
                   array-rank-limit
                   array-dimension-limit
                   array-total-size-limit
                   (upgraded-array-element-type 'base-char)
                   (upgraded-array-element-type '(unsigned-byte 8)))",
        )
        .to_string(),
        "(BIT 9223372036854775807 9223372036854775807 9223372036854775807 CHARACTER (UNSIGNED-BYTE 8))"
    );
    assert_eq!(
        evaluate(
            "(let ((vector (make-array 4
                                      :element-type 'character
                                      :initial-element #\\A
                                      :fill-pointer 2)))
               (list (array-has-fill-pointer-p vector)
                     (fill-pointer vector)
                     (adjustable-array-p vector)
                     (multiple-value-list (array-displacement vector))
                     (array-element-type vector)
                     (simple-vector-p vector)
                     (simple-array-p vector)
                     (typep vector 'vector)
                     (typep vector 'simple-vector)))",
        )
        .to_string(),
        "(T 2 NIL (NIL 0) CHARACTER NIL NIL T NIL)"
    );
    assert_eq!(
        evaluate(
            "(let* ((vector (make-array 4
                                       :element-type 'character
                                       :initial-element #\\A
                                       :fill-pointer 2))
                    (array (make-array '(2 2)
                                       :initial-element 0
                                       :element-type 'integer)))
               (setf (aref vector 1) #\\Z
                     (row-major-aref array 2) 7)
               (list (fill-pointer vector)
                     (array-element-type vector)
                     (aref vector 1)
                     (array-element-type array)
                     (aref array 1 0)))",
        )
        .to_string(),
        "(2 CHARACTER #\\Z INTEGER 7)"
    );
    assert_eq!(
        evaluate(
            "(let ((vector (make-array 4
                                      :initial-contents '(1 2 3 4)
                                      :fill-pointer 3
                                      :element-type 'integer)))
               (setf (fill-pointer vector) 1)
               (list (fill-pointer vector)
                     (array-element-type vector)
                     (aref vector 0)
                     (aref vector 3)))",
        )
        .to_string(),
        "(1 INTEGER 1 4)"
    );
    assert_eq!(
        evaluate(
            "(let ((array (make-array '(2 2) :initial-element 0)))
               (list (adjustable-array-p array)
                     (multiple-value-list (array-displacement array))))",
        )
        .to_string(),
        "(NIL (NIL 0))"
    );
    assert_eq!(
        evaluate(
            "(let* ((base (make-array 5 :initial-contents '(5 1 4 3 2)))
                    (displaced (make-array 3 :displaced-to base :displaced-index-offset 1)))
               (list (sort displaced #'<)
                     (multiple-value-list (array-displacement displaced))
                     (simple-array-p displaced)))",
        )
        .to_string(),
        "(#(1 3 4) (#(5 1 3 4 2) 1) NIL)"
    );
    assert_eq!(
        evaluate(
            "(let* ((base (make-array 5 :initial-contents '(10 20 30 40 50)))
                    (displaced (make-array 3
                                           :displaced-to base
                                           :displaced-index-offset 1
                                           :fill-pointer 2
                                           :element-type 'integer
                                           :adjustable t)))
               (map-into displaced #'1+ '(1 2 3))
               (list displaced
                     (multiple-value-list (array-displacement displaced))
                     (fill-pointer displaced)
                     (adjustable-array-p displaced)
                     (array-element-type displaced)
                     (simple-array-p displaced)))",
        )
        .to_string(),
        "(#(2 3 4) (#(10 2 3 4 50) 1) 2 T INTEGER NIL)"
    );
    assert_eq!(
        evaluate(
            "(let* ((base (make-array 5 :initial-contents '(1 2 3 2 4)))
                    (displaced (make-array 5
                                           :displaced-to base
                                           :fill-pointer 4
                                           :element-type 'integer
                                           :adjustable t)))
               (let ((result (remove 2 displaced :count 1)))
                 (list result
                       (multiple-value-list (array-displacement result))
                       (fill-pointer result)
                       (adjustable-array-p result)
                       (array-element-type result)
                       (simple-array-p result))))",
        )
        .to_string(),
        "(#(1 3 2 4) (NIL 0) 4 T INTEGER NIL)"
    );
    assert_eq!(
        evaluate(
            "(let* ((base (make-array 4 :initial-contents '(1 2 2 3)))
                    (displaced (make-array 4
                                           :displaced-to base
                                           :fill-pointer 3
                                           :element-type 'integer
                                           :adjustable t)))
               (let ((result (substitute 9 2 displaced :count 1)))
                 (list result
                       (multiple-value-list (array-displacement result))
                       (fill-pointer result)
                       (adjustable-array-p result)
                       (array-element-type result)
                       (simple-array-p result))))",
        )
        .to_string(),
        "(#(1 9 2 3) (NIL 0) 3 T INTEGER NIL)"
    );
    assert_eq!(
        evaluate(
            "(let ((vector (make-array 3
                                      :initial-contents '(1 2 3)
                                      :element-type 'integer
                                      :adjustable t)))
               (list (adjustable-array-p vector)
                     (simple-array-p vector)
                     (typep vector 'simple-vector)
                     (array-element-type vector)))",
        )
        .to_string(),
        "(T NIL NIL INTEGER)"
    );
    assert_eq!(
        evaluate(
            "(let* ((vector (make-array 3
                                       :initial-contents '(1 2 3)
                                       :fill-pointer 2
                                       :element-type 'integer
                                       :adjustable t))
                    (adjusted (adjust-array vector 5 :initial-element 9)))
               (list adjusted
                     (adjustable-array-p adjusted)
                     (fill-pointer adjusted)
                     (array-element-type adjusted)
                     (aref adjusted 3)
                     (aref adjusted 4)
                     (adjustable-array-p adjusted)))",
        )
        .to_string(),
        "(#(1 2 3 9 9) T 2 INTEGER 9 9 T)"
    );
    assert_eq!(
        evaluate(
            "(let* ((array (make-array '(2 2)
                                      :initial-contents '((1 2) (3 4))
                                      :element-type 'integer
                                      :adjustable t))
                    (adjusted (adjust-array array '(2 3) :initial-element 0)))
               (list (array-dimensions adjusted)
                     (adjustable-array-p adjusted)
                     (simple-array-p adjusted)
                     (array-element-type adjusted)
                     (aref adjusted 0 0)
                     (aref adjusted 0 1)
                     (aref adjusted 0 2)
                     (aref adjusted 1 0)
                     (aref adjusted 1 1)
                     (aref adjusted 1 2)))",
        )
        .to_string(),
        "((2 3) T NIL INTEGER 1 2 3 4 0 0)"
    );
    assert_eq!(
        evaluate(
            "(let* ((base (make-array 5 :initial-contents '(10 11 12 13 14)))
                    (displaced (make-array 3 :displaced-to base :displaced-index-offset 1)))
               (setf (aref displaced 1) 99)
               (list displaced
                     (multiple-value-list (array-displacement displaced))
                     (simple-array-p displaced)
                     (aref displaced 0)
                     (aref displaced 1)
                     (aref displaced 2)
                     (aref base 2)))",
        )
        .to_string(),
        "(#(11 99 13) (#(10 11 99 13 14) 1) NIL 11 99 13 99)"
    );
    assert_eq!(
        evaluate(
            "(let* ((base (make-array 6 :initial-contents '(0 1 2 3 4 5)))
                    (displaced (make-array '(2 2) :displaced-to base :displaced-index-offset 2))
                    (adjusted (adjust-array displaced '(1 3) :displaced-to base :displaced-index-offset 1)))
               (list (array-dimensions displaced)
                     (row-major-aref displaced 0)
                     (row-major-aref displaced 3)
                     (multiple-value-list (array-displacement displaced))
                     (simple-array-p displaced)
                     (array-dimensions adjusted)
                     (row-major-aref adjusted 0)
                     (row-major-aref adjusted 2)
                     (multiple-value-list (array-displacement adjusted))
                     (simple-array-p adjusted)))",
        )
        .to_string(),
        "((2 2) 2 5 (#(0 1 2 3 4 5) 2) NIL (1 3) 1 3 (#(0 1 2 3 4 5) 1) NIL)"
    );
}

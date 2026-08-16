(define-condition invalid-form-error
  (ncl-error)
  ((form :initarg :form :reader invalid-form))
  (:report
   (lambda (condition stream)
     (format stream "Invalid form: ~S." (invalid-form condition)))))

(define-condition unbound-variable-error
  (ncl-error)
  ((name :initarg :name :reader unbound-variable-name))
  (:report
   (lambda (condition stream)
     (format stream "Unbound variable ~S." (unbound-variable-name condition)))))

(define-condition undefined-function-error
  (ncl-error)
  ((name :initarg :name :reader undefined-function-name))
  (:report
   (lambda (condition stream)
     (format stream
             "Undefined function ~S."
             (undefined-function-name condition)))))

(define-condition arity-error
  (ncl-error)
  ((expected :initarg :expected :reader arity-error-expected)
   (actual :initarg :actual :reader arity-error-actual))
  (:report
   (lambda (condition stream)
     (format stream
             "Wrong number of arguments: expected ~A, got ~D."
             (arity-error-expected condition)
             (arity-error-actual condition)))))

(define-condition macro-expansion-limit-error
  (ncl-error)
  ((form :initarg :form :reader macro-expansion-form))
  (:report
   (lambda (condition stream)
     (format stream
             "Macro expansion limit exceeded for ~S."
             (macro-expansion-form condition)))))

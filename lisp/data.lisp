(in-package #:ncl)

(defstruct (environment (:constructor %make-environment (parent)))
  parent
  (values (make-hash-table :test #'eq))
  (functions (make-hash-table :test #'eq))
  (macros (make-hash-table :test #'eq)))

(defun make-environment (&optional parent)
  (%make-environment parent))

(defun child-environment (environment)
  (make-environment environment))

(defun bind-value (environment name value)
  (setf (gethash name (environment-values environment)) value)
  value)

(defun set-value (environment name value)
  (loop for current = environment then (environment-parent current)
        while current
        do (multiple-value-bind (old-value presentp)
               (gethash name (environment-values current))
             (declare (ignore old-value))
             (when presentp
               (setf (gethash name (environment-values current)) value)
               (return-from set-value
                 value))))
  (bind-value environment name value))

(defun lookup-binding (environment accessor name)
  (loop for current = environment then (environment-parent current)
        while current
        do (multiple-value-bind (value presentp)
               (gethash name (funcall accessor current))
             (when presentp
               (return (values value t))))
        finally (return (values nil nil))))

(defun lookup-value (environment name)
  (lookup-binding environment #'environment-values name))

(defun lookup-named-binding (environment accessor name)
  (multiple-value-bind (value presentp)
      (lookup-binding environment accessor name)
    (if presentp
        (values value t)
        (loop for current = environment then (environment-parent current)
              while current
              do (maphash
                  (lambda (candidate candidate-value)
                    (when
                        (and (symbolp candidate)
                             (symbolp name)
                             (string-equal (symbol-name candidate)
                                           (symbol-name name)))
                      (return-from lookup-named-binding
                        (values candidate-value t))))
                  (funcall accessor current))
              finally (return (values nil nil))))))

(defun define-function (environment name function)
  (setf (gethash name (environment-functions environment)) function)
  function)

(defun lookup-function (environment name)
  (lookup-named-binding environment #'environment-functions name))

(defun define-macro (environment name macro)
  (setf (gethash name (environment-macros environment)) macro)
  macro)

(defun lookup-macro (environment name)
  (lookup-named-binding environment #'environment-macros name))

(defstruct (closure (:constructor make-closure (parameters body environment)))
  parameters
  body
  environment)

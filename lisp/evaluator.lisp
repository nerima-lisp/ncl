(defun proper-list-p (value)
  (labels ((walk (tail seen)
             (cond
               ((null tail) t)
               ((atom tail) nil)
               ((member tail seen :test #'eq) nil)
               (t (walk (cdr tail) (cons tail seen))))))
    (walk value nil)))

(defun operator-named-p (operator name)
  (and (symbolp operator) (string-equal (symbol-name operator) name)))

(defun malformed-form (form)
  (make-condition 'invalid-form-error :form form))

(defun macroexpand-1 (form &optional environment)
  (let ((actual-environment (or environment (make-standard-environment))))
    (cond
      ((and (consp form) (not (proper-list-p form)))
       (error (malformed-form form)))
      ((and (consp form) (symbolp (car form)))
       (multiple-value-bind (macro presentp)
           (lookup-macro actual-environment (car form))
         (if presentp
             (values (invoke-callable macro (cdr form)) t)
             (values form nil))))
      (t
       (values form nil)))))

(defun macroexpand-form (form environment)
  (loop with current = form
        with expandedp = nil
        repeat *max-macro-expansions*
        do (multiple-value-bind (next changedp)
               (macroexpand-1 current environment)
             (unless changedp
               (return (values current expandedp)))
             (setf current next
                   expandedp t))
        finally (error 'macro-expansion-limit-error :form form)))

(defun macroexpand (form &optional environment)
  (macroexpand-form form (or environment (make-standard-environment))))

(defun evaluate-cps (form environment)
  (lambda (success failure)
    (evaluate-form-cps form environment success failure)))

(defun evaluate-form-cps (form environment success failure)
  (handler-case (cond
                  ((or (null form)
                       (eq form t)
                       (keywordp form)
                       (numberp form)
                       (characterp form)
                       (stringp form)) (funcall success form))
                  ((symbolp form)
                   (multiple-value-bind (value presentp)
                       (lookup-value environment form)
                     (if presentp
                         (funcall success value)
                         (funcall failure
                                  (make-condition 'unbound-variable-error
                                                  :name
                                                  form)))))
                  ((not (proper-list-p form))
                   (funcall failure (malformed-form form)))
                  (t
                   (multiple-value-bind (expanded expandedp)
                       (macroexpand-form form environment)
                     (if expandedp
                         (evaluate-form-cps expanded
                                            environment
                                            success
                                            failure)
                         (let ((operator (car form))
                               (arguments (cdr form)))
                           (cond
                             ((operator-named-p operator "QUOTE")
                              (if (= (length arguments) 1)
                                  (funcall success (car arguments))
                                  (funcall failure (malformed-form form))))
                             ((operator-named-p operator "IF")
                              (if (member (length arguments) '(2 3))
                                  (funcall
                                   (evaluate-cps (first arguments) environment)
                                   (lambda (value)
                                     (funcall
                                      (evaluate-cps
                                       (if value
                                           (second arguments)
                                           (if (= (length arguments) 3)
                                               (third arguments)
                                               nil))
                                       environment)
                                      success
                                      failure))
                                   failure)
                                  (funcall failure (malformed-form form))))
                             ((operator-named-p operator "SETQ")
                              (if (and (evenp (length arguments))
                                       (loop for tail on arguments by #'cddr
                                             always (symbolp (first tail))))
                                  (funcall
                                   (evaluate-setq-cps arguments environment)
                                   success
                                   failure)
                                  (funcall failure (malformed-form form))))
                             ((operator-named-p operator "PROGN")
                              (funcall
                               (evaluate-sequence-cps arguments environment)
                               success
                               failure))
                             ((operator-named-p operator "LET")
                              (if (and (>= (length arguments) 1)
                                       (proper-list-p (first arguments)))
                                  (funcall
                                   (evaluate-binding-scope-cps (first arguments)
                                                               (rest arguments)
                                                               environment
                                                               nil)
                                   success
                                   failure)
                                  (funcall failure (malformed-form form))))
                             ((operator-named-p operator "LET*")
                              (if (and (>= (length arguments) 1)
                                       (proper-list-p (first arguments)))
                                  (funcall
                                   (evaluate-binding-scope-cps (first arguments)
                                                               (rest arguments)
                                                               environment
                                                               t)
                                   success
                                   failure)
                                  (funcall failure (malformed-form form))))
                             ((operator-named-p operator "LAMBDA")
                              (if (and (>= (length arguments) 1)
                                       (proper-list-p (first arguments)))
                                  (funcall success
                                           (make-closure (first arguments)
                                                         (rest arguments)
                                                         environment))
                                  (funcall failure (malformed-form form))))
                             ((operator-named-p operator "DEFUN")
                              (if (and (>= (length arguments) 3)
                                       (symbolp (first arguments))
                                       (proper-list-p (second arguments)))
                                  (let ((function
                                         (make-closure (second arguments)
                                                       (cddr arguments)
                                                       environment)))
                                    (define-function environment
                                                     (first arguments)
                                                     function)
                                    (funcall success (first arguments)))
                                  (funcall failure (malformed-form form))))
                             ((operator-named-p operator "DEFMACRO")
                              (if (and (>= (length arguments) 3)
                                       (symbolp (first arguments))
                                       (proper-list-p (second arguments)))
                                  (let ((macro
                                         (make-closure (second arguments)
                                                       (cddr arguments)
                                                       environment)))
                                    (define-macro environment
                                                  (first arguments)
                                                  macro)
                                    (funcall success (first arguments)))
                                  (funcall failure (malformed-form form))))
                             ((operator-named-p operator "FUNCTION")
                              (if (= (length arguments) 1)
                                  (funcall
                                   (evaluate-function-designator-cps
                                    (first arguments)
                                    environment)
                                   success
                                   failure)
                                  (funcall failure (malformed-form form))))
                             (t
                              (funcall
                               (evaluate-call-cps operator
                                                  arguments
                                                  environment)
                               success
                               failure))))))))
    (condition (condition)
      (funcall failure condition))))

(defun evaluate-setq-step-cps (remaining last-value environment success failure)
  (if (endp remaining)
      (funcall success last-value)
      (let ((name (first remaining))
            (form (second remaining)))
        (funcall (evaluate-cps form environment)
                 (lambda (value)
                   (set-value environment name value)
                   (evaluate-setq-step-cps (cddr remaining)
                                            value
                                            environment
                                            success
                                            failure))
                 failure))))

(defun evaluate-setq-cps (arguments environment)
  (lambda (success failure)
    (if (proper-list-p arguments)
        (evaluate-setq-step-cps arguments nil environment success failure)
        (funcall failure (malformed-form arguments)))))

(defun evaluate-sequence-step-cps (remaining last-value environment success failure)
  (if (endp remaining)
      (funcall success last-value)
      (funcall (evaluate-cps (first remaining) environment)
               (lambda (value)
                 (evaluate-sequence-step-cps (rest remaining)
                                              value
                                              environment
                                              success
                                              failure))
               failure)))

(defun evaluate-sequence-cps (forms environment)
  (lambda (success failure)
    (evaluate-sequence-step-cps forms nil environment success failure)))

(defun evaluate-forms-cps (forms environment)
  (evaluate-sequence-cps forms environment))

(defun evaluate-function-designator-cps (designator environment)
  (if (symbolp designator)
      (lambda (success failure)
        (multiple-value-bind (function presentp)
            (lookup-function environment designator)
          (if presentp
              (funcall success function)
              (funcall failure
                       (make-condition 'undefined-function-error
                                       :name
                                       designator)))))
      (evaluate-cps designator environment)))

(defun evaluate-call-cps (operator arguments environment)
  (lambda (success failure)
    (funcall (evaluate-function-designator-cps operator environment)
             (lambda (function)
               (funcall (evaluate-arguments-cps arguments environment)
                        (lambda (values)
                          (invoke-callable-cps function values success failure))
                        failure))
             failure)))

(defun evaluate-arguments-step-cps (remaining values environment success failure)
  (if (endp remaining)
      (funcall success (nreverse values))
      (funcall (evaluate-cps (first remaining) environment)
               (lambda (value)
                 (evaluate-arguments-step-cps (rest remaining)
                                               (cons value values)
                                               environment
                                               success
                                               failure))
               failure)))

(defun evaluate-arguments-cps (arguments environment)
  (lambda (success failure)
    (evaluate-arguments-step-cps arguments nil environment success failure)))

(defun invoke-callable-cps (callable arguments success failure)
  (cond
    ((closure-p callable)
     (funcall (bind-closure-arguments-cps callable arguments)
              (lambda (scope)
                (funcall (evaluate-sequence-cps (closure-body callable) scope)
                         success
                         failure))
              failure))
    ((functionp callable)
     (handler-case (funcall success (apply callable arguments))
       (condition (condition)
         (funcall failure condition))))
    (t
     (funcall failure (make-condition 'undefined-function-error :name callable)))))

(defun invoke-callable (callable arguments)
  (let ((result nil)
        (failure nil))
    (invoke-callable-cps callable
                         arguments
                         (lambda (value)
                           (setf result value))
                         (lambda (condition)
                           (setf failure condition)))
    (if failure
        (error failure)
        result)))

(defun parse-binding-spec (binding)
  (cond
    ((symbolp binding) (values binding '(quote nil)))
    ((and (proper-list-p binding)
          (= (length binding) 2)
          (symbolp (first binding))) (values (first binding) (second binding)))
    (t (error 'invalid-form-error :form binding))))

(defun evaluate-binding-scope-cps (bindings body environment sequentialp)
  (lambda (success failure)
    (let ((scope (child-environment environment)))
      (labels ((advance (remaining)
                 (if (endp remaining)
                     (funcall (evaluate-sequence-cps body scope)
                              success
                              failure)
                     (handler-case (multiple-value-bind (name initializer)
                                       (parse-binding-spec (first remaining))
                                     (funcall
                                      (evaluate-cps initializer
                                                    (if sequentialp
                                                        scope
                                                        environment))
                                      (lambda (value)
                                        (bind-value scope name value)
                                        (advance (rest remaining)))
                                      failure))
                       (condition (condition)
                         (funcall failure condition))))))
        (handler-case (advance bindings)
          (condition (condition)
            (funcall failure condition)))))))

(defun evaluate (form &optional environment)
  (cps-run (evaluate-cps form (or environment (make-standard-environment)))))

(defun evaluate-source (source &optional environment)
  (let ((actual-environment (or environment (make-standard-environment))))
    (cps-run (evaluate-forms-cps (read-forms source) actual-environment))))

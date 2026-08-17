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

(defun evaluate (form &optional environment)
  (cps-run (evaluate-cps form (or environment (make-standard-environment)))))

(defun evaluate-source (source &optional environment)
  (let ((actual-environment (or environment (make-standard-environment))))
    (cps-run (evaluate-forms-cps (read-forms source) actual-environment))))

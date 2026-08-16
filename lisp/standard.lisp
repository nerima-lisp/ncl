(defun install-standard-function (environment name function)
  (define-function environment name function)
  environment)

(defun install-standard-macro (environment name function)
  (define-macro environment name function)
  environment)

(defun install-standard-functions (environment definitions)
  (dolist (definition definitions environment)
    (install-standard-function environment
                               (first definition)
                               (second definition))))

(defun ncl-function-p (value)
  (or (functionp value) (closure-p value)))

(defun install-numeric-functions (environment)
  (install-standard-functions environment
                              (list (list '+ #'+)
                                    (list '- #'-)
                                    (list '* #'*)
                                    (list '/ #'/)
                                    (list '= #'=)
                                    (list '< #'<)
                                    (list '> #'>)
                                    (list '<= #'<=)
                                    (list '>= #'>=)
                                    (list '1+ #'1+)
                                    (list '1- #'1-)
                                    (list 'zerop #'zerop)
                                    (list 'plusp #'plusp)
                                    (list 'minusp #'minusp)
                                    (list 'evenp #'evenp)
                                    (list 'oddp #'oddp)
                                    (list 'abs #'abs)
                                    (list 'min #'min)
                                    (list 'max #'max)
                                    (list 'mod #'mod)
                                    (list 'rem #'rem)
                                    (list 'expt #'expt))))

(defun install-sequence-functions (environment)
  (install-standard-functions environment
                              (list (list 'list #'list)
                                    (list 'list* #'list*)
                                    (list 'cons #'cons)
                                    (list 'car #'car)
                                    (list 'cdr #'cdr)
                                    (list 'first #'first)
                                    (list 'rest #'rest)
                                    (list 'cadr #'cadr)
                                    (list 'caddr #'caddr)
                                    (list 'nth #'nth)
                                    (list 'last #'last)
                                    (list 'butlast #'butlast)
                                    (list 'append #'append)
                                    (list 'length #'length)
                                    (list 'reverse #'reverse)
                                    (list 'member #'member)
                                    (list 'assoc #'assoc)
                                    (list 'getf #'getf))))

(defun install-predicate-functions (environment)
  (install-standard-functions environment
                              (list (list 'null #'null)
                                    (list 'not #'not)
                                    (list 'identity #'identity)
                                    (list 'eq #'eq)
                                    (list 'eql #'eql)
                                    (list 'equal #'equal)
                                    (list 'consp #'consp)
                                    (list 'listp #'listp)
                                    (list 'atom #'atom)
                                    (list 'numberp #'numberp)
                                    (list 'symbolp #'symbolp)
                                    (list 'stringp #'stringp)
                                    (list 'characterp #'characterp)
                                    (list 'functionp #'ncl-function-p))))

(defun install-higher-order-functions (environment)
  (install-standard-function environment
                             'funcall
                             (lambda (function &rest arguments)
                               (invoke-callable function arguments)))
  (install-standard-function environment
                             'apply
                             (lambda (function &rest arguments)
                               (unless arguments
                                 (error 'arity-error :expected 2 :actual 1))
                               (let ((tail (car (last arguments))))
                                 (unless (listp tail)
                                   (error 'invalid-form-error :form tail))
                                 (invoke-callable function
                                                  (append (butlast arguments)
                                                          tail)))))
  (install-standard-function environment
                             'mapcar
                             (lambda (function sequence)
                               (mapcar
                                (lambda (value)
                                  (invoke-callable function (list value)))
                                sequence)))
  (install-standard-function environment 'values #'values)
  environment)

(defun expand-when (condition body)
  (list 'if condition (cons 'progn body) nil))

(defun expand-unless (condition body)
  (list 'if condition nil (cons 'progn body)))

(defun expand-and (forms)
  (cond
    ((null forms) t)
    ((null (rest forms)) (first forms))
    (t (list 'if (first forms) (expand-and (rest forms)) nil))))

(defun expand-or (forms)
  (cond
    ((null forms) nil)
    ((null (rest forms)) (first forms))
    (t
     (let ((value (gensym "OR-VALUE-")))
       (list 'let
             (list (list value (first forms)))
             (list 'if value value (expand-or (rest forms))))))))

(defun expand-cond (clauses)
  (if (null clauses)
      nil
      (let ((clause (first clauses)))
        (unless (and (proper-list-p clause) clause)
          (error (malformed-form clause)))
        (list 'if
              (first clause)
              (if (rest clause)
                  (cons 'progn (rest clause))
                  (first clause))
              (expand-cond (rest clauses))))))

(defun install-standard-macros (environment)
  (install-standard-macro environment
                          'when
                          (lambda (condition &rest body)
                            (expand-when condition body)))
  (install-standard-macro environment
                          'unless
                          (lambda (condition &rest body)
                            (expand-unless condition body)))
  (install-standard-macro environment
                          'and
                          (lambda (&rest forms)
                            (expand-and forms)))
  (install-standard-macro environment
                          'or
                          (lambda (&rest forms)
                            (expand-or forms)))
  (install-standard-macro environment
                          'cond
                          (lambda (&rest clauses)
                            (expand-cond clauses)))
  environment)

(defun make-standard-environment ()
  (let ((environment (make-environment)))
    (install-numeric-functions environment)
    (install-sequence-functions environment)
    (install-predicate-functions environment)
    (install-higher-order-functions environment)
    (install-standard-macros environment)
    environment))

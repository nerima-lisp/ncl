(in-package #:ncl)

(define-condition unknown-keyword-error
  (ncl-error)
  ((name :initarg :name :reader unknown-keyword-name))
  (:report
   (lambda (condition stream)
     (format stream
             "Unknown keyword argument ~S."
             (unknown-keyword-name condition)))))

(defstruct
    (lambda-parameter
     (:constructor make-lambda-parameter (name initform supplied-p)))
  name
  initform
  supplied-p)

(defstruct
    (lambda-key-parameter
     (:constructor make-lambda-key-parameter (keyword name initform supplied-p)))
  keyword
  name
  initform
  supplied-p)

(defstruct
    (lambda-list-spec
     (:constructor make-lambda-list-spec
                   (required optional
                             rest-name
                             keys
                             key-section-p
                             allow-other-keys
                             aux)))
  required
  optional
  rest-name
  keys
  key-section-p
  allow-other-keys
  aux)

(defun lambda-marker-p (value name)
  (and (symbolp value) (string-equal (symbol-name value) name)))

(defun normalize-optional-parameter (parameter lambda-list)
  (cond
    ((symbolp parameter) (make-lambda-parameter parameter nil nil))
    ((and (proper-list-p parameter)
          (member (length parameter) '(1 2 3))
          (symbolp (first parameter)))
     (make-lambda-parameter (first parameter)
                            (second parameter)
                            (third parameter)))
    (t (error 'invalid-form-error :form lambda-list))))

(defun keyword-for-name (name)
  (intern (symbol-name name) (find-package '#:keyword)))

(defun normalize-key-parameter (parameter lambda-list)
  (cond
    ((symbolp parameter)
     (make-lambda-key-parameter (keyword-for-name parameter) parameter nil nil))
    ((proper-list-p parameter)
     (let ((length (length parameter))
           (head (first parameter)))
       (cond
         ((and (member length '(1 2 3)) (symbolp head))
          (make-lambda-key-parameter (keyword-for-name head)
                                     head
                                     (second parameter)
                                     (third parameter)))
         ((and (proper-list-p head)
               (= (length head) 2)
               (keywordp (first head))
               (symbolp (second head))
               (member length '(1 2 3)))
          (make-lambda-key-parameter (first head)
                                     (second head)
                                     (second parameter)
                                     (third parameter)))
         (t (error 'invalid-form-error :form lambda-list)))))
    (t (error 'invalid-form-error :form lambda-list))))

(defun normalize-aux-parameter (parameter lambda-list)
  (cond
    ((symbolp parameter) (make-lambda-parameter parameter nil nil))
    ((and (proper-list-p parameter)
          (member (length parameter) '(1 2))
          (symbolp (first parameter)))
     (make-lambda-parameter (first parameter) (second parameter) nil))
    (t (error 'invalid-form-error :form lambda-list))))

(defun parse-lambda-list (lambda-list)
  (unless (proper-list-p lambda-list)
    (error 'invalid-form-error :form lambda-list))
  (let ((state :required)
        (required nil)
        (optional nil)
        (rest-name nil)
        (keys nil)
        (key-section-p nil)
        (allow-other-keys nil)
        (aux nil))
    (dolist (parameter lambda-list)
      (cond
        ((lambda-marker-p parameter "&OPTIONAL")
          (unless (eq state :required)
            (error 'invalid-form-error :form lambda-list))
          (setf state :optional))
        ((or (lambda-marker-p parameter "&REST")
             (lambda-marker-p parameter "&BODY"))
          (unless (member state '(:required :optional))
            (error 'invalid-form-error :form lambda-list))
          (setf state :rest))
        ((lambda-marker-p parameter "&KEY")
          (unless (member state '(:required :optional :after-rest))
            (error 'invalid-form-error :form lambda-list))
          (setf state :key
                key-section-p t))
        ((lambda-marker-p parameter "&ALLOW-OTHER-KEYS")
          (unless (eq state :key)
            (error 'invalid-form-error :form lambda-list))
          (when allow-other-keys
            (error 'invalid-form-error :form lambda-list))
          (setf allow-other-keys t))
        ((lambda-marker-p parameter "&AUX")
          (unless (member state '(:required :optional :after-rest :key))
            (error 'invalid-form-error :form lambda-list))
          (setf state :aux))
        ((eq state :rest)
         (if (symbolp parameter)
             (setf rest-name parameter
                   state :after-rest)
             (error 'invalid-form-error :form lambda-list)))
        ((eq state :required)
         (if (symbolp parameter)
             (push parameter required)
             (error 'invalid-form-error :form lambda-list)))
        ((eq state :optional)
         (push (normalize-optional-parameter parameter lambda-list) optional))
        ((eq state :key)
         (push (normalize-key-parameter parameter lambda-list) keys))
        ((eq state :aux)
         (push (normalize-aux-parameter parameter lambda-list) aux))
        (t (error 'invalid-form-error :form lambda-list))))
    (when (eq state :rest)
      (error 'invalid-form-error :form lambda-list))
    (make-lambda-list-spec (nreverse required)
                           (nreverse optional)
                           rest-name
                           (nreverse keys)
                           key-section-p
                           allow-other-keys
                           (nreverse aux))))

(defun parse-keyword-arguments (arguments)
  (unless (evenp (length arguments))
    (error 'invalid-form-error :form arguments))
  (loop for tail on arguments by #'cddr
        for keyword = (first tail)
        for value = (second tail)
        unless (keywordp keyword)
          do (error 'invalid-form-error :form arguments)
        collect (cons keyword value)))

(defun find-keyword-argument (keyword arguments)
  (let ((entry (assoc keyword arguments :test #'eq)))
    (if entry
        (values (cdr entry) t)
        (values nil nil))))

(defun find-key-parameter (keyword parameters)
  (find keyword parameters :key #'lambda-key-parameter-keyword :test #'eq))

(defun bind-auxiliary-arguments-cps (parameters scope success failure)
  (if (endp parameters)
      (funcall success)
      (let ((parameter (first parameters)))
        (funcall (evaluate-cps (lambda-parameter-initform parameter) scope)
                 (lambda (value)
                   (bind-value scope (lambda-parameter-name parameter) value)
                   (bind-auxiliary-arguments-cps (rest parameters)
                                                 scope
                                                 success
                                                 failure))
                 failure))))

(defun bind-optional-arguments-cps (parameters remaining scope success failure)
  (if (endp parameters)
      (funcall success remaining)
      (let ((parameter (first parameters)))
        (if remaining
            (progn
              (bind-value scope
                          (lambda-parameter-name parameter)
                          (pop remaining))
              (when (lambda-parameter-supplied-p parameter)
                (bind-value scope (lambda-parameter-supplied-p parameter) t))
              (bind-optional-arguments-cps (rest parameters)
                                           remaining
                                           scope
                                           success
                                           failure))
            (funcall (evaluate-cps (lambda-parameter-initform parameter) scope)
                     (lambda (value)
                       (bind-value scope
                                   (lambda-parameter-name parameter)
                                   value)
                       (when (lambda-parameter-supplied-p parameter)
                         (bind-value scope
                                     (lambda-parameter-supplied-p parameter)
                                     nil))
                       (bind-optional-arguments-cps (rest parameters)
                                                    remaining
                                                    scope
                                                    success
                                                    failure))
                     failure)))))

(defun validate-keyword-arguments (keyword-arguments parameters
                                                     allow-other-keys)
  (dolist (argument keyword-arguments)
    (let ((keyword (car argument)))
      (unless
          (or (eq keyword :allow-other-keys)
              allow-other-keys
              (find-key-parameter keyword parameters))
        (error 'unknown-keyword-error :name keyword)))))

(defun bind-keyword-arguments-cps (parameters keyword-arguments
                                              scope
                                              success
                                              failure)
  (if (endp parameters)
      (funcall success)
      (let ((parameter (first parameters)))
        (multiple-value-bind (value presentp)
            (find-keyword-argument (lambda-key-parameter-keyword parameter)
                                   keyword-arguments)
          (if presentp
              (progn
                (bind-value scope (lambda-key-parameter-name parameter) value)
                (when (lambda-key-parameter-supplied-p parameter)
                  (bind-value scope
                              (lambda-key-parameter-supplied-p parameter)
                              t))
                (bind-keyword-arguments-cps (rest parameters)
                                            keyword-arguments
                                            scope
                                            success
                                            failure))
              (funcall
               (evaluate-cps (lambda-key-parameter-initform parameter) scope)
               (lambda (default-value)
                 (bind-value scope
                             (lambda-key-parameter-name parameter)
                             default-value)
                 (when (lambda-key-parameter-supplied-p parameter)
                   (bind-value scope
                               (lambda-key-parameter-supplied-p parameter)
                               nil))
                 (bind-keyword-arguments-cps (rest parameters)
                                             keyword-arguments
                                             scope
                                             success
                                             failure))
               failure))))))

(defun bind-closure-arguments-cps (closure arguments)
  (lambda (success failure)
    (handler-case (let* ((spec (parse-lambda-list (closure-parameters closure)))
                         (scope
                          (child-environment (closure-environment closure)))
                         (remaining arguments))
                    (if (< (length remaining)
                           (length (lambda-list-spec-required spec)))
                        (funcall failure
                                 (make-condition 'arity-error
                                                 :expected
                                                 (length
                                                  (lambda-list-spec-required
                                                   spec))
                                                 :actual
                                                 (length arguments)))
                        (progn
                          (dolist (name (lambda-list-spec-required spec))
                            (bind-value scope name (pop remaining)))
                          (bind-optional-arguments-cps
                           (lambda-list-spec-optional spec)
                           remaining
                           scope
                           (lambda (remaining)
                             (when (lambda-list-spec-rest-name spec)
                               (bind-value scope
                                           (lambda-list-spec-rest-name spec)
                                           remaining))
                             (if (lambda-list-spec-key-section-p spec)
                                 (let* ((keyword-arguments
                                         (parse-keyword-arguments remaining))
                                        (allow-other-keys
                                         (or
                                          (lambda-list-spec-allow-other-keys
                                           spec)
                                          (multiple-value-bind (value presentp)
                                              (find-keyword-argument
                                               :allow-other-keys
                                               keyword-arguments)
                                            (and presentp value)))))
                                   (validate-keyword-arguments keyword-arguments
                                                               (lambda-list-spec-keys
                                                                spec)
                                                               allow-other-keys)
                                   (bind-keyword-arguments-cps
                                    (lambda-list-spec-keys spec)
                                    keyword-arguments
                                    scope
                                    (lambda ()
                                      (bind-auxiliary-arguments-cps
                                       (lambda-list-spec-aux spec)
                                       scope
                                       (lambda ()
                                         (funcall success scope))
                                       failure))
                                    failure))
                                 (if (and remaining
                                          (null (lambda-list-spec-rest-name spec)))
                                     (funcall failure
                                              (make-condition 'arity-error
                                                              :expected
                                                              (length
                                                               (lambda-list-spec-required
                                                                spec))
                                                              :actual
                                                              (length arguments)))
                                     (bind-auxiliary-arguments-cps
                                      (lambda-list-spec-aux spec)
                                      scope
                                      (lambda ()
                                        (funcall success scope))
                                      failure))))
                           failure))))
      (condition (condition)
        (funcall failure condition)))))

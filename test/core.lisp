(in-package #:ncl/test)

(describe "NCL data and control"
          (it "keeps lexical bindings in parent and child environments"
              (let ((parent (ncl:make-environment))
                    (child nil))
                (ncl:bind-value parent 'x 1)
                (setf child (ncl:child-environment parent))
                (ncl:bind-value child 'x 2)
                (expect (multiple-value-list (ncl:lookup-value parent 'x))
                        :to-equal
                        '(1 t))
                (expect (multiple-value-list (ncl:lookup-value child 'x))
                        :to-equal
                        '(2 t))))
          (it "composes computations through CPS"
              (expect
               (ncl:cps-run
                (ncl:cps-bind (ncl:cps-return 2)
                              (lambda (value)
                                (ncl:cps-return (* value 3)))))
               :to-be
               6)
              (expect
               (ncl:cps-run
                (ncl:cps-sequence (ncl:cps-return :first)
                                  (ncl:cps-return :last)))
               :to-be
               :last)
              (expect (ncl:cps-run (ncl:cps-sequence)) :to-be nil)))

(describe "NCL reader and evaluator"
          (it "reads a sequence of forms"
              (expect (ncl:read-forms "(+ 1 2) (quote ready)" :ncl/test)
                      :to-equal
                      '((+ 1 2) (quote ready))))
          (it "evaluates lexical scope, assignment, and macros"
              (expect (ncl:evaluate-source "(let ((x 2)) (setq x (+ x 3)) x)")
                      :to-be
                      5)
              (expect
               (ncl:evaluate-source
                "(progn (defmacro twice (form) (list '+ form form)) (twice 7))")
               :to-be
               14))
          (it "evaluates closures, recursion, and optional arguments"
              (expect
               (ncl:evaluate-source "((lambda (x &optional (y 2)) (+ x y)) 5)")
               :to-be
               7)
              (expect
               (ncl:evaluate-source
                "(progn
         (defun factorial (n)
           (if (<= n 1) 1 (* n (factorial (- n 1)))))
         (factorial 6))")
               :to-be
               720))
          (it "supports keyword and auxiliary lambda parameters"
              (expect-form
               "((lambda (value
                &key (scale 2 scale-p)
                     ((:offset offset) 0 offset-p)
                &aux (total (* value scale)))
         (list total scale-p offset-p offset))
       3 :scale 4 :offset 5)"
               '(12 t t 5))
              (expect-value
               "((lambda (&key value &allow-other-keys) value)
       :value 4 :ignored 9)"
               4))
          (it "supports function designators and list application"
              (expect
               (ncl:evaluate-source
                "(progn
         (defun add-two (x y) (+ x y))
         (apply (function add-two) (list 3 4)))")
               :to-be
               7)))

(describe "NCL standard functions and macros"
          (it "preserves values and short-circuits boolean macros"
              (expect-value "(and 1 2 3)" 3)
              (expect-value "(or nil nil 8)" 8)
              (expect-value "(or 7 missing-variable)" 7)
              (expect-value "(and)" t)
              (expect-value "(or)" nil))
          (it "expands conditional macros"
              (expect-value "(when t 9)" 9)
              (expect-value "(unless nil 10)" 10)
              (expect-value "(cond ((= 1 2) 10) ((= 2 2) 20) (t 30))" 20)
              (multiple-value-bind (form expandedp)
                  (ncl:macroexpand-1 '(and 1 2))
                (declare (ignore form))
                (expect expandedp :to-be t)))
          (it "provides direct numeric, sequence, and higher-order functions"
              (expect-form "(list (1+ 4) (cadr (list 1 2 3)) (evenp 4))"
                           '(5 2 t))
              (expect-form
               "(mapcar (lambda (value) (* value value)) (list 1 2 3))"
               '(1 4 9))))

(describe "NCL conditions"
          (it "reports an unbound variable"
              (expect-condition ncl:unbound-variable-error
                                (ncl:evaluate-source "missing-variable")))
          (it "reports condition details"
              (expect (format nil "~A"
                              (make-condition 'ncl:unbound-variable-error
                                              :name :missing))
                      :to-equal
                      "Unbound variable :MISSING.")
              (expect (format nil "~A"
                              (make-condition 'ncl:undefined-function-error
                                              :name :missing-function))
                      :to-equal
                      "Undefined function :MISSING-FUNCTION.")
              (expect (format nil "~A"
                              (make-condition 'ncl:arity-error
                                              :expected 1
                                              :actual 2))
                      :to-equal
                      "Wrong number of arguments: expected 1, got 2.")
              (expect (format nil "~A"
                              (make-condition 'ncl:macro-expansion-limit-error
                                              :form :loop))
                      :to-equal
                      "Macro expansion limit exceeded for :LOOP.")
              (expect (format nil "~A"
                              (make-condition 'ncl:unknown-keyword-error
                                              :name :other))
                      :to-equal
                      "Unknown keyword argument :OTHER."))
          (it "reports an invalid call arity"
              (expect-condition ncl:arity-error
                                (ncl:evaluate-source "((lambda (x) x))")))
          (it "reports an unknown keyword argument"
              (expect-condition ncl:unknown-keyword-error
                                (ncl:evaluate-source
                                 "((lambda (&key value) value) :other 1)")))
          (it "bounds recursive macro expansion"
              (expect-condition ncl:macro-expansion-limit-error
                                (ncl:evaluate-source
                                 "(progn (defmacro loop () (list 'loop)) (loop))")))
          (it "propagates macro invocation conditions through evaluation"
              (expect-condition ncl:arity-error
                                (ncl:evaluate-source
                                 "(progn (defmacro requires-one (form) form)
                                         (requires-one))"))))

(describe "NCL environment and CPS edge paths"
          (it "updates inherited values and resolves named definitions"
              (let* ((parent (ncl:make-environment))
                     (child (ncl:child-environment parent))
                     (function
                      (lambda (value)
                        value))
                     (macro
                      (lambda (form)
                        form))
                     (other-name (make-symbol "FUNCTION")))
                (ncl:bind-value parent 'value 1)
                (expect (ncl:set-value child 'value 2) :to-be 2)
                (expect (ncl:lookup-value parent 'value) :to-be 2)
                (expect (ncl:set-value child 'new-value 3) :to-be 3)
                (expect
                 (multiple-value-list (ncl:lookup-value parent 'new-value))
                 :to-equal
                 '(nil nil))
                (ncl:define-function parent 'function function)
                (ncl:define-macro parent 'macro macro)
                (let ((non-symbol-environment (ncl:make-environment)))
                  (ncl:define-function non-symbol-environment
                                       "FUNCTION"
                                       function)
                  (expect
                   (multiple-value-list
                    (ncl:lookup-function non-symbol-environment other-name))
                   :to-equal
                   '(nil nil)))
                (expect
                 (multiple-value-list (ncl:lookup-function child other-name))
                 :to-equal
                 (list function t))
                (expect (multiple-value-list (ncl:lookup-function parent 42))
                        :to-equal
                        '(nil nil))
                (expect
                 (multiple-value-list
                  (ncl:lookup-macro child (make-symbol "MACRO")))
                 :to-equal
                 (list macro t))))
          (it "propagates CPS failures and binds CPS values"
              (expect
               (ncl:cps-run (ncl:cps-fail :failed) :on-failure #'identity)
               :to-be
               :failed)
              (expect
               (ncl:cps-run
                (ncl:cps-bind (ncl:cps-fail :failed)
                              (lambda (value)
                                (declare (ignore value))
                                (ncl:cps-return :unreachable)))
                :on-failure
                #'identity)
               :to-be
               :failed)
              (expect (ncl:cps-run (ncl:cps-sequence (ncl:cps-return :only)))
                      :to-be
                      :only)
              (expect
               (ncl:cps-run
                (ncl:cps-let (value (ncl:cps-return 4))
                             (ncl:cps-return (* value 2))))
               :to-be
               8))
          (it "handles cyclic lists and malformed CPS input"
              (let ((cycle (list 'value)))
                (setf (cdr cycle) cycle)
                (expect (ncl::proper-list-p cycle) :to-be nil))
              (expect-condition ncl:invalid-form-error
                                (ncl:cps-run
                                 (ncl::evaluate-setq-cps
                                  '(value . broken)
                                  (ncl:make-standard-environment))))))

(describe "NCL reader and core evaluation paths"
          (it "accepts package designators and empty input"
              (expect (ncl:read-forms "" :ncl/test) :to-equal nil)
              (expect (ncl:read-forms "ready" (find-package :ncl/test))
                      :to-equal
                      (list 'ready))
              (expect (ncl:read-forms "ready" "ncl/test")
                      :to-equal
                      (list 'ready))
              (expect (ncl:read-forms "ready" "NCL/TEST")
                      :to-equal
                      (list 'ready))
              (expect-condition error
                                (ncl:read-forms "ready" "missing-package"))
              (expect-condition error
                                (ncl:read-forms "ready" 'missing-package))
              (expect-condition error
                                (ncl:read-forms "ready" 42)))
          (it "evaluates literals, branching, sequential bindings, and forms"
              (expect-value "nil" nil)
              (expect-value "t" t)
              (expect-value ":ready" :ready)
              (expect-value "42" 42)
              (expect-value "#\\A" #\A)
              (expect (ncl:evaluate-source "\"ready\"")
                      :to-equal
                      "ready")
              (let ((value (ncl:evaluate-source "(quote (ready . now))")))
                (expect (and (consp value)
                             (symbolp (car value))
                             (symbolp (cdr value))
                             (string-equal (symbol-name (car value)) "READY")
                             (string-equal (symbol-name (cdr value)) "NOW"))
                        :to-be
                        t))
              (expect-value "(if nil 1 2)" 2)
              (expect-value "(if t 3)" 3)
              (expect-value "(if nil 3)" nil)
              (expect-value "(let* ((x 1) (y (+ x 2))) y)" 3)
              (expect-value "(let ((x 0) (y 0)) (setq x 2 y 3) (+ x y))" 5)
              (expect-value "(progn)" nil)
              (expect-value
               "(funcall (function (lambda (value) (+ value 1))) 4)"
               5))
          (it "evaluates direct forms and rejects malformed forms"
              (expect (ncl:evaluate '(+ 2 3)) :to-be 5)
              (expect
               (ncl:cps-run
                (ncl:evaluate-forms-cps '((+ 1 1) (+ 2 2))
                                        (ncl:make-standard-environment)))
               :to-be
               4)
              (expect-invalid-sources
                "(if . t)"
                "(if t)"
                "(if t 1 2 3)"
                "(quote)"
                "(quote one two)"
                "(setq x)"
                "(let ((x 1 2)) x)")
              (expect-value "(let (x) x)" nil)
              (expect-invalid-sources
                "(let)"
                "(let* )"
                "(setq 1 2)"
                "(let (x . y) x)"
                "(let* (x . y) x)"
                "((lambda (x . y) nil))"
                "(lambda)"
                "(defun one)"
                "(defun 1 () nil)"
                "(defun one (x . y) nil)"
                "(defmacro one)"
                "(defmacro 1 () nil)"
                "(defmacro one (x . y) nil)"
                "(function)"
                "(function one two)")
              (expect-condition ncl:undefined-function-error
                                (ncl:evaluate-source "(function missing-function)"))
              (expect-condition ncl:undefined-function-error
                                (ncl:evaluate-source "(1)")))
          (it "expands macros to a fixed point and preserves ordinary forms"
              (multiple-value-bind (form expandedp) (ncl:macroexpand-1 '(and 7))
                (expect form :to-be 7)
                (expect expandedp :to-be t))
              (expect (ncl:macroexpand '(and 7)) :to-be 7)
              (multiple-value-bind (form expandedp)
                  (ncl:macroexpand-1 '(ordinary 7))
                (expect form :to-equal '(ordinary 7))
                (expect expandedp :to-be nil))))

(describe "NCL lambda-list paths"
          (it "binds optional, rest, body, and default keyword parameters"
              (expect-form
               "((lambda (value &optional (extra 2 supplied-p))
                   (list value extra supplied-p))
                 3)"
               '(3 2 nil))
              (expect-form
               "((lambda (value &optional (extra 2 supplied-p))
                   (list value extra supplied-p))
                 3 4)"
               '(3 4 t))
              (expect (equal (ncl:evaluate-source
                              "((lambda (&rest values) values) 1 2)")
                             '(1 2))
                      :to-be
                      t)
              (expect (equal (ncl:evaluate-source
                              "((lambda (&body values) values) 1 2)")
                             '(1 2))
                      :to-be
                      t)
              (expect-form
               "((lambda (&key (value 4 supplied-p))
                   (list value supplied-p)))"
               '(4 nil))
              (expect-form
               "((lambda (&key (value 4 supplied-p))
                   (list value supplied-p))
                 :value 9)"
               '(9 t))
              (expect-value "((lambda (&key (value 4)) value))" 4)
              (expect-form
               "((lambda (&key (value 4)) value) :value 9)"
               9)
              (expect-form
               "((lambda (&key ((:value value) 7 supplied-p))
                   (list value supplied-p)))"
               '(7 nil))
              (expect-form
               "((lambda (&key ((:value value) 7)) value) :value 9)"
               9))
          (it "combines rest and keyword arguments and rejects invalid calls"
              (expect-form
               "((lambda (&rest all &key value) (list all value)) :value 3)"
               '((:value 3) 3))
              (expect-condition ncl:arity-error
                                (ncl:evaluate-source
                                 "((lambda (value) value) 1 2)"))
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source "((lambda (&rest) nil))"))
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source
                                 "((lambda (&key value) value) :value)"))
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source
                                 "((lambda (&key value) value) (quote value) 1)")))
          (it "rejects malformed lambda-list parameters and states"
              (expect-value "((lambda (&optional y) y) 4)" 4)
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source
                                 "((lambda (&optional (x 1 2 3)) x))"))
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source
                                 "((lambda (&key (1 2)) nil))"))
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source
                                 "((lambda (&key 1) nil))"))
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source
                                 "((lambda (&key (value 1 2 3)) nil))"))
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source
                                 "((lambda (&key ((value name) 7)) nil))"))
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source
                                 "((lambda (&key ((:value value extra) 7)) nil))"))
              (expect-value "((lambda (&aux x) x))" nil)
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source
                                 "((lambda (&aux (x 1 2)) x))"))
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source
                                 "((lambda (&optional &optional x) nil))"))
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source
                                 "((lambda (&key x &rest rest) nil))"))
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source
                                 "((lambda (&aux x &key y) nil))"))
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source
                                 "((lambda (&optional &allow-other-keys) nil))"))
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source
                                 "((lambda (&key &allow-other-keys &allow-other-keys)
                                     nil))"))
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source
                                 "((lambda (&aux x &aux y) nil))"))
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source
                                 "((lambda (&rest (x)) nil))"))
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source
                                 "((lambda (1) nil))"))
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source
                                 "((lambda (&rest args extra) nil))"))
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source
                                 "((lambda (&optional . x) nil))"))
              (expect-condition ncl:invalid-form-error
                                (ncl::parse-lambda-list '(value . rest)))))

(describe "NCL standard library edge paths"
          (it "provides arithmetic, sequence, and predicate operations"
              (expect-form
               "(list (1- 4) (1+ 4) (zerop 0) (plusp 2) (minusp -1)
                      (max 1 3) (min 1 3) (abs -4) (mod 5 2) (rem 5 2)
                      (expt 2 3))"
               '(3 5 t t t 3 1 4 1 1 8))
              (expect-form
               "(list (list* 1 2 (list 3 4))
                      (cons 1 (list 2))
                      (car (list 1 2))
                      (cdr (list 1 2))
                      (first (list 3 4))
                      (rest (list 3 4))
                      (caddr (list 1 2 3))
                      (nth 1 (list 4 5))
                      (last (list 6 7))
                      (butlast (list 8 9))
                      (append (list 1) (list 2))
                      (length (list 1 2))
                      (reverse (list 1 2)))"
               '((1 2 3 4) (1 2) 1 (2) 3 (4) 3 5 (7) (8) (1 2) 2 (2 1)))
              (expect-form
               "(list (null nil) (not nil) (eq :ready :ready)
                      (eql 2 2) (equal (list 1) (list 1))
                      (consp (list 1)) (listp (list 1)) (atom 1)
                      (numberp 1) (symbolp 'ready) (stringp \"ready\")
                      (characterp #\\A) (functionp (function car))
                      (functionp (function (lambda (value) value)))
                      (functionp 1))"
               '(t t t t t t t t t t t t t t nil)))
          (it "applies callable values and handles application errors"
              (expect-value "(funcall (function +) 2 3)" 5)
              (expect-value "(apply (function +) (list 2 3))" 5)
              (expect-form "(mapcar (function 1+) (list 1 2 3))" '(2 3 4))
              (expect-condition ncl:arity-error
                                (ncl:evaluate-source "(apply (function +))"))
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source "(apply (function +) 1 2)")))
          (it "covers single-clause conditional macro paths"
              (expect-value "(and 8)" 8)
              (expect-value "(or 8)" 8)
              (expect-value "(cond)" nil)
              (expect-value "(cond (t))" t)
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source "(cond ())"))
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source "(cond broken)"))
              (expect-condition ncl:invalid-form-error
                                (ncl:evaluate-source "(cond . broken)"))
              (expect-condition ncl:invalid-form-error
                                (ncl:macroexpand-1 '(cond . broken)))
              (expect-condition ncl:invalid-form-error
                                (ncl:macroexpand-1 '(cond ((t) . broken))))))

(describe "NCL command-line paths"
          (it "prints results, help, and command-line evaluations"
              (let ((output (make-string-output-stream)))
                (expect (ncl::print-result :ready output) :to-be :ready)
                (expect (get-output-stream-string output)
                        :to-equal
                        (format nil ":READY~%"))
                (let ((*standard-output* output))
                  (expect (ncl:run-command-line '("--eval" "(+ 2 3)")) :to-be 5))
                (expect (get-output-stream-string output)
                        :to-equal
                        (format nil "5~%"))
                (let ((*standard-output* output))
                  (expect (ncl:run-command-line '("--help")) :to-be t))
                (expect (not (null (search "Usage:"
                                            (get-output-stream-string output))))
                        :to-be
                        t)))
          (it "reports command-line errors and runs a file"
              (let ((error-output (make-string-output-stream))
                    (output (make-string-output-stream))
                    (pathname
                     (merge-pathnames
                      (format nil "ncl-test-~D.lisp" (get-universal-time))
                      (pathname "/tmp/"))))
                (unwind-protect
                    (progn
                      (with-open-file
                          (stream pathname
                                  :direction
                                  :output
                                  :if-exists
                                  :supersede)
                        (write-string "(+ 4 5)" stream))
                      (let ((*standard-output* output))
                        (expect
                         (ncl:run-command-line
                          (list "--file" (namestring pathname)))
                         :to-be
                         9))
                      (expect (get-output-stream-string output)
                              :to-equal
                              (format nil "9~%"))
                      (let ((*error-output* error-output))
                        (expect (ncl:run-command-line '("--eval")) :to-be nil)
                        (expect (ncl:run-command-line '("--unknown"))
                                :to-be
                                nil))
                      (let ((errors (get-output-stream-string error-output)))
                        (expect (not (null (search "requires a form" errors)))
                                :to-be
                                t)
                        (expect (not (null (search "Unknown option" errors)))
                                :to-be
                                t))
                      (let ((version-output (make-string-output-stream)))
                        (let ((*standard-output* version-output))
                          (expect (ncl:run-command-line '("--version"))
                                  :to-be
                                  t))
                        (expect (search "ncl "
                                        (get-output-stream-string version-output))
                                :to-be
                                0))
                      (let ((*error-output* error-output))
                        (expect (ncl:run-command-line '("--file")) :to-be nil)
                        (expect
                         (ncl:run-command-line
                          (list "--file"
                                (namestring
                                 (merge-pathnames
                                  (format nil "ncl-missing-~D.lisp"
                                          (get-universal-time))
                                  (pathname "/tmp/")))))
                         :to-be
                         nil))
                      (let ((errors (get-output-stream-string error-output)))
                        (expect (search "--file requires a path" errors)
                                :to-be
                                0)
                        (expect (not (null (search "Error:" errors)))
                                :to-be
                                t)))
                  (when (probe-file pathname)
                    (delete-file pathname)))))
          (it "runs REPL modes and exposes argv parsing"
              (let ((input (make-string-input-stream
                            (format nil "(+ 1 2)~%")))
                    (output (make-string-output-stream)))
                (let ((*standard-input* input)
                      (*standard-output* output))
                  (expect (ncl:run-command-line nil) :to-be t))
                (expect (search "ncl> 3" (get-output-stream-string output))
                        :to-be
                        0))
              (let ((input (make-string-input-stream
                            (format nil "(+ 1 2)~%")))
                    (output (make-string-output-stream))
                    (sb-ext:*posix-argv*
                      '("sbcl" "--script" "run-tests.lisp" "--repl")))
                (let ((*standard-input* input)
                      (*standard-output* output))
                  (expect (ncl:run-command-line) :to-be t))
                (expect (search "ncl> 3" (get-output-stream-string output))
                        :to-be
                        0))
              (let ((input (make-string-input-stream
                            (format nil "(+ 1 2)~%")))
                    (output (make-string-output-stream)))
                (let ((*standard-input* input)
                      (*standard-output* output))
                  (expect (ncl:run-command-line '("--repl")) :to-be t))
                (expect (search "ncl> 3" (get-output-stream-string output))
                        :to-be
                        0))
              (let ((input (make-string-input-stream
                            (format nil "(if . t)~%")))
                    (output (make-string-output-stream))
                    (error-output (make-string-output-stream)))
                (let ((*standard-input* input)
                      (*standard-output* output)
                      (*error-output* error-output))
                  (expect (ncl:run-command-line '("--repl")) :to-be t))
                (expect (search "Error:"
                                (get-output-stream-string error-output))
                        :to-be
                        0))
              (let ((sb-ext:*posix-argv*
                     '("sbcl" "--script" "run.lisp" "--eval" "(+ 1 2)")))
                (expect (ncl::command-line-arguments)
                        :to-equal
                        '("--eval" "(+ 1 2)")))
              (let ((sb-ext:*posix-argv* '("sbcl" "--" "--help")))
                (expect (ncl::command-line-arguments) :to-equal '("--help")))
              (let ((sb-ext:*posix-argv* '("sbcl" "--help")))
                (expect (ncl::command-line-arguments) :to-equal '("--help")))))

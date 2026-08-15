(defpackage #:ncl
            (:use #:cl)
            (:shadow #:macroexpand-1 #:macroexpand)
            (:export #:environment
                     #:make-environment
                     #:child-environment
                     #:bind-value
                     #:set-value
                     #:lookup-value
                     #:define-function
                     #:lookup-function
                     #:define-macro
                     #:lookup-macro
                     #:closure
                     #:make-closure
                     #:closure-parameters
                     #:closure-body
                     #:closure-environment
                     #:cps-return
                     #:cps-fail
                     #:cps-bind
                     #:cps-run
                     #:cps-sequence
                     #:cps-let
                     #:read-forms
                     #:ncl-error
                     #:invalid-form-error
                     #:invalid-form
                     #:unbound-variable-error
                     #:unbound-variable-name
                     #:undefined-function-error
                     #:undefined-function-name
                     #:arity-error
                     #:arity-error-expected
                     #:arity-error-actual
                     #:unknown-keyword-error
                     #:unknown-keyword-name
                     #:macro-expansion-limit-error
                     #:macro-expansion-form
                     #:*version*
                     #:evaluate-cps
                     #:evaluate-forms-cps
                     #:evaluate
                     #:evaluate-source
                     #:macroexpand-1
                     #:macroexpand
                     #:make-standard-environment
                     #:run-command-line))

(in-package #:asdf-user)

(defsystem "ncl"
  :description "A CPS-first Common Lisp core for NCL."
  :version "0.1.0"
  :license "MIT"
  :serial t
  :around-compile
  (lambda (thunk)
    (let ((package (find-package "NCL")))
      (unless package
        (error "The NCL package must be available before compiling components."))
      (let ((*package* package))
        (funcall thunk))))
  :pathname "lisp/"
  :components ((:file "package" :around-compile nil)
               (:file "constants")
               (:file "data")
               (:file "logic")
               (:file "cps-macros")
               (:file "conditions-base")
               (:file "reader")
               (:file "conditions")
               (:file "evaluator")
               (:file "evaluator-dispatch")
               (:file "lambda-list")
               (:file "standard")
               (:file "cli")))

(defsystem "ncl/test"
  :description "Executable tests for the NCL Common Lisp core."
  :depends-on ("ncl" "cl-weave")
  :serial t
  :pathname "test/"
  :components ((:file "package") (:file "support") (:file "core")))

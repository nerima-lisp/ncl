(in-package #:asdf-user)

(defsystem "ncl"
  :description "A CPS-first Common Lisp core for NCL."
  :version "0.1.0"
  :license "MIT"
  :serial t
  :pathname "lisp/"
  :components ((:file "package") (:file "data")
                                 (:file "logic")
                                 (:file "reader")
                                 (:file "conditions")
                                 (:file "evaluator")
                                 (:file "lambda-list")
                                 (:file "standard")
                                 (:file "cli")))

(defsystem "ncl/test"
  :description "Executable tests for the NCL Common Lisp core."
  :depends-on ("ncl" "cl-weave")
  :serial t
  :pathname "test/"
  :components ((:file "package") (:file "support") (:file "core")))

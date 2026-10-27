;;;; SBCL cl-bench driver.
(load "package.lisp")

(in-package :cl-bench)

(export '(defbench bench-run-1 bench-run *misc-dir*) (find-package "CL-BENCH"))

(progn
  (defun benchmark-gc ()
    (sb-ext:gc :full t))
  (defparameter *disabled-benchmarks*
    '("WALK-LIST/MESS"))
  (load
   (make-pathname :name "driver-core" :type "lisp" :defaults *load-truename*)))

;; tests.lisp supplies only benchmark metadata and function designators.  It
;; is safe to load after the replacement DEFBENCH has been installed.

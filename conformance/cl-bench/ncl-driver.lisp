;;;; NCL cl-bench driver.
;;;;
;;;; This driver intentionally does not load upstream cl-bench.lisp.  The
;;;; following upstream functions/macros are replaced here:
;;;;   DEFBENCH, BENCH-RUN-1, BENCH-RUN, timing, result recording, and GC.
;;;; The benchmark bodies remain the pinned files/*.lisp sources.
(load "package.lisp")

(in-package :cl-bench)

(export '(defbench bench-run-1 bench-run *misc-dir*) (find-package "CL-BENCH"))

(progn
  (defun benchmark-gc ()
    (funcall (intern "GC" (find-package "NCL-GC"))))
  (defparameter *disabled-benchmarks*
    nil)
  (load
   (make-pathname :name "driver-core" :type "lisp" :defaults *load-truename*)))

;; tests.lisp supplies only benchmark metadata and function designators.  It
;; is safe to load after the replacement DEFBENCH has been installed.

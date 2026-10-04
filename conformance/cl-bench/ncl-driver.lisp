;;;; NCL cl-bench driver.
;;;;
;;;; This driver intentionally does not load upstream cl-bench.lisp.  The
;;;; following upstream functions/macros are replaced here:
;;;;   DEFBENCH, BENCH-RUN-1, BENCH-RUN, timing, result recording, and GC.
;;;; The benchmark bodies remain the pinned files/*.lisp sources.

(load "package.lisp")

(in-package :cl-bench)

(export '(defbench bench-run-1 bench-run *misc-dir*)
        (find-package "CL-BENCH"))

(defparameter *misc-dir* "files/")
(defparameter *benchmarks* nil)
(defparameter *results* nil)
(defparameter *load-results* nil)
(defparameter *load-failed* nil)

(defun ncl-gc ()
  ;; NCL's collector is deliberately called through its NCL extension.  This
  ;; is not an ASDF or trivial-garbage compatibility shim.
  (funcall (intern "GC" (find-package "NCL-GC"))))

(defmacro defbench (name &rest options)
  (let ((function (getf options :function))
        (runs (or (getf options :runs) 1))
        (setup (getf options :setup)))
    `(push (list ',name ,function ,runs ,setup) *benchmarks*)))

(defun elapsed-seconds (start finish)
  (/ (float (- finish start)) internal-time-units-per-second))

(defun bench-run-1 (benchmark)
  (let* ((name (first benchmark))
         (function (second benchmark))
         (runs (third benchmark))
         (setup (fourth benchmark))
         (samples nil))
    (with-open-file (progress "/tmp/c2-clbench-bench-progress"
                              :direction :output
                              :if-exists :append
                              :if-does-not-exist :create)
      (format progress "BENCH ~A status=started~%" name))
    (format *error-output* "BENCH ~A status=started~%" name)
    (finish-output *error-output*)
    (dotimes (run runs)
      (declare (ignore run))
      (when setup (funcall setup))
      (ncl-gc)
      (let ((start (get-internal-real-time)))
        (funcall function)
        (push (elapsed-seconds start (get-internal-real-time)) samples)))
    (push (list name (nreverse samples)) *results*)
    (with-open-file (progress "/tmp/c2-clbench-bench-progress"
                              :direction :output
                              :if-exists :append
                              :if-does-not-exist :create)
      (format progress "BENCH ~A status=passed seconds=~{~A~^,~}~%"
              name (nreverse samples)))
    (format *error-output* "BENCH ~A status=passed seconds=~{~A~^,~}~%"
            name (nreverse samples))
    (finish-output *error-output*)
    (car (last samples))))

(defun bench-run ()
  (setf *results* nil)
  (dolist (benchmark (nreverse *benchmarks*))
    (bench-run-1 benchmark))
  (format t "{\"times\":[~{~A~^,~}]}~%"
          (mapcan #'second (nreverse *results*)))
  *results*)

(defun load-benchmark-file (file)
  (let ((start (get-internal-real-time)))
    ;; The fixed cl-bench layout is a relative namestring accepted by LOAD.
    (format *error-output* "LOAD ~A status=started~%" file)
    (finish-output *error-output*)
    (load (concatenate 'string *misc-dir* file))
    (push (list file :loaded
                (elapsed-seconds start (get-internal-real-time)))
          *load-results*)
    :loaded))

(dolist (file '("arrays.lisp"
                "bignum.lisp"
                "boehm-gc.lisp"
                "clos.lisp"
                "clos-janderson.lisp"
                "crc40.lisp"
                "deflate.lisp"
                "gabriel.lisp"
                "hash.lisp"
                "math.lisp"
                "misc.lisp"
                "ratios.lisp"
                "richards.lisp"))
  (load-benchmark-file file))

;; tests.lisp supplies only benchmark metadata and function designators.  It
;; is safe to load after the replacement DEFBENCH has been installed.
(load-benchmark-file "tests.lisp")
(when *benchmarks* (bench-run))

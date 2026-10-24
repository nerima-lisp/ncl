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

(progn
  (defun elapsed-seconds (start finish)
    (/ (float (- finish start)) internal-time-units-per-second))

  (defun benchmark-name-text (name)
    (if (stringp name) name (symbol-name name)))

  (defun write-json-string (value stream)
    (write-char #\" stream)
    (loop for character across value
          do (cond
              ((char= character #\") (write-string "\\\"" stream))
              ((char= character #\\) (write-string "\\\\" stream))
              ((char= character #\Newline) (write-string "\\n" stream))
              ((char= character #\Tab) (write-string "\\t" stream))
              ((= (char-code character) 13) (write-string "\\r" stream))
              ((< (char-code character) 32)
               (format stream "\\u~4,'0X" (char-code character)))
              (t (write-char character stream))))
    (write-char #\" stream))

  (defun write-json-samples (samples stream)
    (write-char #\[ stream)
    (loop for sample in samples
          for first = t then nil
          do (unless first (write-char #\, stream))
          (format stream "~,12F" (coerce sample 'double-float)))
    (write-char #\] stream))

  (defun write-json-result (results stream)
    (let* ((failed (or *load-failed*
                       (null results)
                       (some (lambda (result)
                               (eq (second result) :failed))
                             results)))
           (flat-times
            (loop for result in results
                  when (eq (second result) :passed)
                  append (copy-list (third result)))))
      (write-string "{\"status\":" stream)
      (write-json-string (if failed "failed" "passed") stream)
      (write-string ",\"times\":" stream)
      (write-json-samples flat-times stream)
      (write-string ",\"benchmarks\":{" stream)
      (loop for result in results
            for first = t then nil
            do (unless first (write-char #\, stream))
            (write-json-string
             (benchmark-name-text (first result)) stream)
            (write-string ":{\"status\":" stream)
            (write-json-string
             (string-downcase (symbol-name (second result))) stream)
            (write-string ",\"times\":" stream)
            (write-json-samples (third result) stream)
            (write-string ",\"error\":" stream)
            (if (fourth result)
                (write-json-string (fourth result) stream)
              (write-string "null" stream))
            (write-char #\} stream))
      (write-string "}}" stream)
      (terpri stream)
      (finish-output stream))))

(defun bench-run-1 (benchmark)
  (let* ((name (first benchmark))
         (function (second benchmark))
         (runs (third benchmark))
         (setup (fourth benchmark))
         (samples nil)
         (failure nil))
    (dotimes (run runs)
      (declare (ignore run))
      (unless failure
        (handler-case
            (progn
              (when setup (funcall setup))
              (ncl-gc)
              (let ((start (get-internal-real-time)))
                (funcall function)
                (push (elapsed-seconds start (get-internal-real-time))
                      samples)))
          (error (condition)
            (setf failure (princ-to-string condition))))))
    (push (list name
                (if failure :failed :passed)
                (nreverse samples)
                failure)
          *results*)
    (unless failure
      (car (last (third (first *results*)))))))

(defun bench-run ()
  (setf *results* nil)
  (dolist (benchmark (nreverse *benchmarks*))
    (bench-run-1 benchmark))
  (setf *results* (nreverse *results*))
  (write-json-result *results* *standard-output*)
  *results*)

(defun load-benchmark-file (file &optional (base-directory *misc-dir*))
  (let ((start (get-internal-real-time)))
    (block load-file
      (handler-bind ((error
                       (lambda (condition)
                         (setf *load-failed* t)
                         (push (list file :failed (princ-to-string condition))
                               *load-results*)
                         (return-from load-file :failed))))
        (load (merge-pathnames file base-directory)))
      (push (list file :loaded
                  (elapsed-seconds start (get-internal-real-time)))
            *load-results*)
      :loaded)))

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
(load-benchmark-file "tests.lisp" #p"./")
(progn
  (dolist (result (nreverse *load-results*))
    (when (eq (second result) :failed)
      (format *error-output* "failed to load ~A: ~A~%"
              (first result) (third result))))
  (bench-run)
  (when (or *load-failed*
            (null *results*)
            (some (lambda (result)
                    (eq (second result) :failed))
                  *results*))
    (error "cl-bench benchmark execution failed")))

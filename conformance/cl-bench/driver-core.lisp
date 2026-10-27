(in-package :cl-bench)

(defparameter *misc-dir* "files/")
(defparameter *source-files*
  (if (boundp 'common-lisp-user::*cl-bench-source-files*)
      common-lisp-user::*cl-bench-source-files*
      '("arrays.lisp" "bignum.lisp" "boehm-gc.lisp" "clos.lisp"
        "clos-janderson.lisp" "crc40.lisp" "deflate.lisp" "gabriel.lisp"
        "hash.lisp" "math.lisp" "misc.lisp" "ratios.lisp" "richards.lisp")))
(defparameter *benchmarks* nil)
(defparameter *benchmark-order* nil)
(defparameter *registration-failures* nil)
(defparameter *global-registration-failures* nil)
(defparameter *results* nil)
(defparameter *load-results* nil)
(defparameter *load-failed* nil)
(defvar *benchmark-selection*
  (if (boundp 'common-lisp-user::*cl-bench-selection*)
      common-lisp-user::*cl-bench-selection*
      nil))
(defvar *driver-mode*
  (if (boundp 'common-lisp-user::*cl-bench-driver-mode*)
      common-lisp-user::*cl-bench-driver-mode*
      :run))
(defvar *preflight-file*
  (if (boundp 'common-lisp-user::*cl-bench-preflight-file*)
      common-lisp-user::*cl-bench-preflight-file*
      nil))
(defvar *skip-files*
  (if (boundp 'common-lisp-user::*cl-bench-skip-files*)
      common-lisp-user::*cl-bench-skip-files*
      nil))
(defvar *skip-benchmarks*
  (if (boundp 'common-lisp-user::*cl-bench-skip-benchmarks*)
      common-lisp-user::*cl-bench-skip-benchmarks*
      nil))
(defvar *disabled-benchmarks* nil)

(defmacro defbench (name &rest options)
  (let ((function (getf options :function))
        (runs (or (getf options :runs) 1))
        (setup (getf options :setup))
        (display-name (or (getf options :short) `(quote ,name)))
        (row-name (gensym "ROW-NAME")))
    `(let ((,row-name (benchmark-name-text ,display-name)))
       (push ,row-name *benchmark-order*)
       (emit-stage-marker "register" ,row-name)
       (handler-case
           (push (list ,row-name ,function ,runs ,setup) *benchmarks*)
         (error (condition)
           (push (list ,row-name (princ-to-string condition) nil)
                 *registration-failures*))))))

(defun elapsed-seconds (start finish)
  (/ (float (- finish start)) internal-time-units-per-second))
(defun emit-stage-marker (stage detail) (format *error-output* "CL-BENCH-STAGE ~A ~A~%" stage detail) (finish-output *error-output*))

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

(defun result-failed-p (results)
  (or *load-failed*
      (null results)
      (some (lambda (result) (eq (second result) :failed)) results)))

(defun write-json-result (results stream)
  (let ((flat-times
          (loop for result in results
                when (eq (second result) :passed)
                  append (copy-list (fourth result)))))
    (write-string "{\"status\":" stream)
    (write-json-string (if (result-failed-p results) "failed" "passed") stream)
    (write-string ",\"times\":" stream)
    (write-json-samples flat-times stream)
    (write-string ",\"benchmarks\":{" stream)
    (loop for result in results
          for first = t then nil
          do (unless first (write-char #\, stream))
             (write-json-string (first result) stream)
             (write-string ":{\"status\":" stream)
             (write-json-string
              (string-downcase (symbol-name (second result))) stream)
             (write-string ",\"stage\":" stream)
             (write-json-string
              (string-downcase (symbol-name (third result))) stream)
             (write-string ",\"times\":" stream)
             (write-json-samples (fourth result) stream)
             (write-string ",\"condition\":" stream)
             (if (fifth result)
                 (write-json-string (fifth result) stream)
                 (write-string "null" stream))
             (write-char #\} stream))
    (write-string "}}" stream)
    (terpri stream)
    (finish-output stream)))

(defun load-registration-file ()
  (emit-stage-marker "register" "tests.lisp")
  (handler-case
      (load "tests.lisp")
    (error (condition)
      (push (princ-to-string condition) *global-registration-failures*))))

(defun load-benchmark-file (file)
  (emit-stage-marker "load" file)
  (let ((start (get-internal-real-time)))
    (handler-case
        (progn
          (load (merge-pathnames file *misc-dir*))
          (push (list file :loaded
                      (elapsed-seconds start (get-internal-real-time)))
                *load-results*)
          :loaded)
      (error (condition)
        (setf *load-failed* t)
        (push (list file :failed (princ-to-string condition)) *load-results*)
        :failed))))

(defun preflight-benchmark-source ()
  (let ((found nil))
    (dolist (file *source-files*)
      (unless (member file *skip-files* :test #'string-equal)
        (when (eq (load-benchmark-file file) :failed)
          (error "cl-bench source load failed for ~A: ~A"
                 file (third (first *load-results*)))))
      (when (string-equal file *preflight-file*)
        (setf found t)
        (return)))
    (unless found
      (error "unknown cl-bench preflight source: ~A" *preflight-file*))))

(defun load-passing-benchmark-sources ()
  (dolist (file *source-files*)
    (unless (member file *skip-files* :test #'string-equal)
      (load-benchmark-file file))))

(defun benchmark-selected-p (name)
  (and (or (null *benchmark-selection*)
           (member name *benchmark-selection* :test #'string-equal))
       (not (member name *skip-benchmarks* :test #'string-equal))))

(defun benchmark-function-text (function)
  (and function (princ-to-string function)))

(defun bench-run-1 (benchmark)
  (let* ((name (benchmark-name-text (first benchmark)))
         (function (second benchmark))
         (runs (third benchmark))
         (setup (fourth benchmark))
         (function-text (benchmark-function-text function))
         (samples nil)
         (failure nil))
    (if (member name *disabled-benchmarks* :test #'string-equal)
        (progn
          (emit-stage-marker "finish" name)
          (list name :disabled :finish nil
                "disabled on this implementation" function-text))
        (progn
          (emit-stage-marker "execute" name)
          (dotimes (run runs)
            (unless failure
              (handler-case
                  (progn
                    (when setup (funcall setup))
                    (benchmark-gc)
                    (let ((start (get-internal-real-time)))
                      (funcall function)
                      (push (elapsed-seconds start (get-internal-real-time))
                            samples)))
                (error (condition)
                  (setf failure (princ-to-string condition))))))
          (unless failure
            (emit-stage-marker "finish" name))
          (list name
                (if failure :failed :passed)
                (if failure :execute :finish)
                (nreverse samples)
                failure
                function-text)))))

(defun bench-run ()
  (setf *results* nil)
  (let* ((registered (nreverse *benchmarks*))
         (registration-failures (nreverse *registration-failures*))
         (global-registration-failures
           (nreverse *global-registration-failures*))
         (names (or *benchmark-selection*
                    (nreverse *benchmark-order*)))
         (global-registration-condition
           (when global-registration-failures
             (format nil "~{~A~^; ~}" global-registration-failures))))
    (dolist (name names)
      (when (benchmark-selected-p name)
        (let ((registration-failure
                (assoc name registration-failures :test #'string-equal))
              (benchmark
                (find name registered
                      :key (lambda (entry)
                             (benchmark-name-text (first entry)))
                      :test #'string-equal)))
          (push
           (cond
             (global-registration-condition
              (emit-stage-marker "register" name)
              (list name :failed :register nil
                    global-registration-condition nil))
             (registration-failure
              (list name :failed :register nil
                    (second registration-failure)
                    (third registration-failure)))
             (benchmark (bench-run-1 benchmark))
             (t
              (list name :failed :register nil
                    "benchmark registration produced no entry"
                    nil)))
           *results*))))
    (setf *results* (nreverse *results*)))
  (write-json-result *results* *standard-output*)
  *results*)

(defun run-cl-bench ()
  (if (eq *driver-mode* :preflight)
      (preflight-benchmark-source)
      (progn
        (load-passing-benchmark-sources)
        (load-registration-file)
        (bench-run)
        (when (result-failed-p *results*)
          (error "cl-bench benchmark execution failed")))))

(run-cl-bench)

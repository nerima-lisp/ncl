(defpackage #:ncl-surface
  (:use #:cl))
(in-package #:ncl-surface)

(defparameter *root* (merge-pathnames "conformance/" (truename "./")))
(defparameter *symbols-dir* (merge-pathnames "sbcl/symbols/" *root*))

(defun yes/no (value) (if value "yes" "no"))
(defun safe-find-class (symbol)
  (handler-case (and (find-class symbol nil) t)
    (error () nil)))
(defun safe-type-kind (symbol)
  (handler-case
      (let ((kind (sb-int:info :type :kind symbol)))
        (yes/no (member kind '(:defined :primitive :instance :structure :condition)
                       :test #'eq)))
    (error () "no")))
(defun type-flags (symbol)
  (let ((class (safe-find-class symbol))
        (f (fboundp symbol))
        (value (boundp symbol)))
    (list (yes/no (and f (macro-function symbol)))
          (yes/no (and f (special-operator-p symbol)))
          (yes/no (and f (not (macro-function symbol))
                       (not (special-operator-p symbol))))
          (yes/no (and value (constantp symbol)))
          (yes/no (and value (not (constantp symbol))))
          (yes/no class)
          (yes/no (and class (handler-case (subtypep symbol 'condition)
                               (error () nil))))
          (yes/no (and (eq (sb-int:info :type :kind symbol) :type) t)))))
(defun row (symbol package)
  (handler-case
      (destructuring-bind (macro special function constant variable class condition type)
          (type-flags symbol)
        (format nil "~A~C~A~C~A~C~A~C~A~C~A~C~A~C~A~C~A~C~A~C~A~C~A~C~A~C~A~%"
                (package-name package) #\Tab (symbol-name symbol) #\Tab
                function #\Tab macro #\Tab special #\Tab variable #\Tab
                constant #\Tab type #\Tab class #\Tab condition #\Tab
                (yes/no (fboundp symbol)) #\Tab (yes/no (boundp symbol)) #\Tab
                (yes/no (safe-find-class symbol)) #\Tab (safe-type-kind symbol)))
    (error (condition)
      (format nil "~A~C~A~Cerror~Cno~Cno~Cno~Cno~Cno~Cno~Cno~Cno~Cno~Cno~C~A~%"
              (package-name package) #\Tab (symbol-name symbol) #\Tab
              #\Tab #\Tab #\Tab #\Tab #\Tab #\Tab #\Tab #\Tab #\Tab #\Tab
              condition))))
(defparameter *header*
  (format nil "package~Csymbol~Cfunction~Cmacro~Cspecial-operator~Cvariable~Cconstant~Ctype~Cclass~Ccondition~Cfboundp~Cboundp~Cfind-class~Csb-int-type-kind~%"
          #\Tab #\Tab #\Tab #\Tab #\Tab #\Tab #\Tab #\Tab #\Tab #\Tab #\Tab #\Tab #\Tab))
(defun write-package (package-name)
  (let* ((package (find-package package-name))
         (file (merge-pathnames
                (format nil "~A.tsv" (string-downcase package-name)) *symbols-dir*)))
    (when package
      (with-open-file (stream file :direction :output :if-exists :supersede)
        (write-string *header* stream)
        (do-external-symbols (symbol package)
          (write-string (row symbol package) stream)))))
  (values))
(defun try-require (name)
  (handler-case (progn (require name) (values t nil))
    (error (condition)
      (values nil (map 'string (lambda (character)
                                 (if (member character '(#\Newline #\Return #\Tab))
                                     #\Space
                                     character))
                       (princ-to-string condition))))))
(defparameter *contribs*
  '(("sb-posix" "SB-POSIX") ("sb-bsd-sockets" "SB-BSD-SOCKETS")
    ("sb-concurrency" "SB-CONCURRENCY") ("sb-queue" "SB-QUEUE")
    ("sb-md5" "SB-MD5") ("sb-rotate-byte" "SB-ROTATE-BYTE")
    ("sb-cltl2" "SB-CLTL2") ("sb-gmp" "SB-GMP") ("sb-mpfr" "SB-MPFR")
    ("sb-simd" "SB-SIMD") ("sb-capstone" "SB-CAPSTONE")
    ("sb-aclrepl" "SB-ACLREPL") ("sb-executable" "SB-EXECUTABLE")
    ("sb-rt" "SB-RT") ("sb-grovel" "SB-GROVEL") ("sb-perf" "SB-PERF")
    ("sb-sprof" "SB-SPROF") ("sb-cover" "SB-COVER")
    ("sb-introspect" "SB-INTROSPECT") ("sb-simple-streams" "SB-SIMPLE-STREAMS")
    ("asdf" "ASDF") ("uiop" "UIOP")))
(defun contribs ()
  (with-open-file (stream (merge-pathnames "contribs.tsv" *symbols-dir*)
                         :direction :output :if-exists :supersede)
    (format stream "name~Cpackage~Cstatus~Cexternal-count~Creason~%" #\Tab #\Tab #\Tab #\Tab)
    (dolist (entry *contribs*)
      (destructuring-bind (name package-name) entry
        (multiple-value-bind (loaded reason) (try-require name)
          (let ((package (find-package package-name)))
            (if (and loaded package)
                (let ((path (merge-pathnames (format nil "contrib-~A.tsv" name) *symbols-dir*)))
                  (with-open-file (out path :direction :output :if-exists :supersede)
                    (write-string *header* out)
                    (do-external-symbols (symbol package)
                      (write-string (row symbol package) out)))
                  (format stream "~A~C~A~Cloaded~C~D~C-~%" name #\Tab package-name #\Tab #\Tab
                          (count-external-symbols package) #\Tab))
                (format stream "~A~C~A~Cfailed~C0~C~A~%" name #\Tab package-name #\Tab #\Tab #\Tab
                        (or reason "package not found")))))))))
(defun count-external-symbols (package)
  (let ((count 0)) (do-external-symbols (symbol package count) (declare (ignore symbol)) (incf count))))
(ensure-directories-exist *symbols-dir*)
(dolist (package '("COMMON-LISP" "SB-EXT" "SB-THREAD" "SB-ALIEN" "SB-SYS"
                   "SB-MOP" "SB-GRAY" "SB-UNICODE" "SB-DEBUG"))
  (write-package package))
(contribs)
(format t "SBCL ~A~%" (lisp-implementation-version))
(sb-ext:quit :unix-status 0)

(defpackage #:ncl-surface (:use #:cl))

(in-package #:ncl-surface)

(require "sb-introspect")

(defparameter *script-directory*
  (make-pathname :name nil :type nil :defaults *load-truename*))

(defparameter *symbols-directory*
  (merge-pathnames "symbols/" *script-directory*))

(defparameter *header*
  '("package" "symbol" "present" "external" "kind" "lambda-list"))

(defparameter *kind-order*
  '("special-operator" "macro"
                       "function"
                       "variable"
                       "constant"
                       "class"
                       "condition"
                       "type"
                       "other"))

(defparameter *public-packages*
  '("COMMON-LISP" "SB-EXT"
                  "SB-THREAD"
                  "SB-ALIEN"
                  "SB-SYS"
                  "SB-MOP"
                  "SB-GRAY"
                  "SB-UNICODE"
                  "SB-DEBUG"))

(defparameter *sbcl-contribs*
  '(("sb-posix" "SB-POSIX") ("sb-bsd-sockets" "SB-BSD-SOCKETS")
    ("sb-concurrency" "SB-CONCURRENCY")
    ("sb-queue" "SB-QUEUE")
    ("sb-md5" "SB-MD5")
    ("sb-rotate-byte" "SB-ROTATE-BYTE")
    ("sb-cltl2" "SB-CLTL2")
    ("sb-gmp" "SB-GMP")
    ("sb-mpfr" "SB-MPFR")
    ("sb-simd" "SB-SIMD")
    ("sb-capstone" "SB-CAPSTONE")
    ("sb-aclrepl" "SB-ACLREPL")
    ("sb-executable" "SB-EXECUTABLE")
    ("sb-rt" "SB-RT")
    ("sb-grovel" "SB-GROVEL")
    ("sb-perf" "SB-PERF")
    ("sb-sprof" "SB-SPROF")
    ("sb-cover" "SB-COVER")
    ("sb-introspect" "SB-INTROSPECT")
    ("sb-simple-streams" "SB-SIMPLE-STREAMS")))

(defparameter *additional-systems*
  '(("asdf" "ASDF") ("uiop" "UIOP")))

(defun join-with-tab (fields)
  (with-output-to-string (stream)
    (loop for field in fields
          for first = t then nil
          unless first
            do (write-char #\Tab stream)
          do (princ field stream))))

(defun write-row (stream fields)
  (write-string (join-with-tab fields) stream)
  (terpri stream))

(defun split-tsv-line (line)
  (let ((start 0)
        (fields nil))
    (loop for
          end = (position #\Tab line :start start)
          do (push (subseq line start end) fields)
          if
          end
          do (setf start (1+ end))
          else
            do (return))
    (nreverse fields)))

(defun read-internal-references ()
  (with-open-file
      (stream (merge-pathnames "internal-referenced.tsv" *symbols-directory*)
              :direction
              :input
              :external-format
              :utf-8)
    (let* ((header-line (read-line stream nil nil))
           (header (and header-line (split-tsv-line header-line)))
           (source-header
            (list "package"
                  "symbol"
                  "libraries"
                  "example-file"
                  "exported-in-sbcl-2.6.0"))
           (surface-header
            (append *header*
                    (list "libraries" "example-file" "exported-in-sbcl-2.6.0")))
           (schema
            (cond
              ((equal header source-header) :source)
              ((equal header surface-header) :surface)
              (t (error "Malformed internal-reference header")))))
      (loop for line = (read-line stream nil nil)
            while line
            collect (let ((fields (split-tsv-line line)))
                      (ecase schema
                        (:source
                          (unless
                              (and (= (length fields) 5)
                                   (not (string= (first fields) ""))
                                   (not (string= (second fields) "")))
                            (error "Malformed internal-reference row"))
                          fields)
                        (:surface
                          (unless
                              (and (= (length fields) 9)
                                   (not (string= (first fields) ""))
                                   (not (string= (second fields) ""))
                                   (or (string= (third fields) "yes")
                                       (string= (third fields) "no"))
                                   (or (string= (fourth fields) "yes")
                                       (string= (fourth fields) "no"))
                                   (not (string= (fifth fields) ""))
                                   (not (string= (sixth fields) "")))
                            (error "Malformed generated internal-reference row"))
                          (append (subseq fields 0 2) (subseq fields 6 9)))))))))

(defun safe-find-class (symbol)
  (handler-case (and (find-class symbol nil) t)
    (error ()
      nil)))

(defun safe-type-kind (symbol)
  (handler-case (sb-int:info :type :kind symbol)
    (error ()
      nil)))

(defun condition-class-p (symbol)
  (and (safe-find-class symbol)
       (handler-case (multiple-value-bind (condition-p certain-p)
                         (subtypep symbol 'condition)
                       (and certain-p condition-p))
         (error ()
           nil))))

(defun constant-symbol-p (symbol)
  (handler-case (and (boundp symbol) (constantp symbol))
    (error ()
      nil)))

(defun symbol-kinds (symbol)
  (let* ((function-bound (fboundp symbol))
         (macro-p (and function-bound (macro-function symbol)))
         (special-p (special-operator-p symbol))
         (variable-p (and (boundp symbol) (not (constant-symbol-p symbol))))
         (constant-p (constant-symbol-p symbol))
         (class-p (safe-find-class symbol))
         (condition-p (condition-class-p symbol))
         (type-p
          (member (safe-type-kind symbol)
                  '(:defined :primitive :instance :structure :condition)
                  :test
                  #'eq))
         (roles
          (remove nil
                  (list (and special-p "special-operator")
                        (and macro-p "macro")
                        (and function-bound
                             (not macro-p)
                             (not special-p)
                             "function")
                        (and variable-p "variable")
                        (and constant-p "constant")
                        (and class-p "class")
                        (and condition-p "condition")
                        (and type-p "type")))))
    (if roles
        (remove-if-not
         (lambda (kind)
           (member kind roles :test #'string=))
         *kind-order*)
        '("other"))))

(defun introspected-lambda-list (function)
  (let* ((package (find-package "SB-INTROSPECT"))
         (symbol (and package (find-symbol "FUNCTION-LAMBDA-LIST" package))))
    (if symbol
        (funcall (symbol-function symbol) function)
        :unknown)))

(defun printed-lambda-list (lambda-list)
  (cond
    ((null lambda-list) "()")
    ((not (listp lambda-list)) "unknown")
    (t
     (handler-case (with-standard-io-syntax
                       (let ((*package* (find-package :cl-user))
                             (*print-case* :downcase)
                             (*print-pretty* nil)
                             (*print-escape* t)
                             (*print-readably* t)
                             (*print-circle* t)
                             (*print-gensym* t)
                             (*print-length* nil)
                             (*print-level* nil))
                         (write-to-string lambda-list)))
       (error ()
         "unknown")))))

(defun function-lambda-list (symbol kinds)
  (if (or (member "function" kinds :test #'string=)
          (member "macro" kinds :test #'string=))
      (handler-case (let ((function
                           (if (member "macro" kinds :test #'string=)
                               (macro-function symbol)
                               (symbol-function symbol))))
                      (printed-lambda-list (introspected-lambda-list function)))
        (error ()
          "unknown"))
      "-"))

(defun present-surface-fields (package-name symbol status)
  (let ((kinds (symbol-kinds symbol)))
    (list package-name
          (symbol-name symbol)
          "yes"
          (if (eq status :external)
              "yes"
              "no")
          (format nil "~{~A~^+~}" kinds)
          (function-lambda-list symbol kinds))))

(defun missing-surface-fields (package-name symbol-name)
  (list package-name symbol-name "no" "no" "other" "-"))

(defun lookup-surface-fields (package-name symbol-name)
  (let ((package (find-package package-name)))
    (if package
        (multiple-value-bind (symbol status) (find-symbol symbol-name package)
          (if status
              (present-surface-fields package-name symbol status)
              (missing-surface-fields package-name symbol-name)))
        (missing-surface-fields package-name symbol-name))))

(defun external-symbols (package)
  (let ((symbols nil))
    (do-external-symbols (symbol package) (push symbol symbols))
    (sort symbols #'string< :key #'symbol-name)))

(defun table-path (filename)
  (merge-pathnames filename *symbols-directory*))

(defun write-symbol-table (package-name)
  (let* ((package
          (or (find-package package-name) (error "Required package is missing")))
         (symbols (external-symbols package))
         (path
          (table-path (format nil "~A.tsv" (string-downcase package-name)))))
    (with-open-file
        (stream path
                :direction
                :output
                :if-exists
                :supersede
                :if-does-not-exist
                :create
                :external-format
                :utf-8)
      (write-row stream *header*)
      (dolist (symbol symbols)
        (multiple-value-bind (found status)
            (find-symbol (symbol-name symbol) package)
          (unless (eq status :external)
            (error "External symbol disappeared during extraction"))
          (write-row stream (present-surface-fields package-name found status)))))
    (length symbols)))

(defun write-internal-references (references)
  (with-open-file
      (stream (table-path "internal-referenced.tsv")
              :direction
              :output
              :if-exists
              :supersede
              :if-does-not-exist
              :create
              :external-format
              :utf-8)
    (write-row stream
               (append *header*
                       '("libraries" "example-file" "exported-in-sbcl-2.6.0")))
    (dolist (fields references)
      (destructuring-bind (package-name symbol-name
                                        libraries
                                        example-file
                                        exported) fields
        (write-row stream
                   (append (lookup-surface-fields package-name symbol-name)
                           (list libraries example-file exported)))))))

(defun x86-64-linux-p ()
  (and (member :x86-64 *features*) (member :linux *features*)))

(defun contrib-in-scope-p (name)
  (or (not (member name '("sb-simd" "sb-perf") :test #'string=))
      (x86-64-linux-p)))

(defun try-require (name)
  (handler-case (progn
                  (require name)
                  t)
    (error ()
      nil)))

(defun remove-stale-contrib-table (name)
  (let ((path (table-path (format nil "contrib-~A.tsv" name))))
    (when (probe-file path)
      (delete-file path))))

(defun write-contrib-table (name package-name package)
  (let ((symbols (external-symbols package))
        (path (table-path (format nil "contrib-~A.tsv" name))))
    (with-open-file
        (stream path
                :direction
                :output
                :if-exists
                :supersede
                :if-does-not-exist
                :create
                :external-format
                :utf-8)
      (write-row stream *header*)
      (dolist (symbol symbols)
        (multiple-value-bind (found status)
            (find-symbol (symbol-name symbol) package)
          (unless (eq status :external)
            (error "External symbol disappeared during extraction"))
          (write-row stream (present-surface-fields package-name found status))))
      (length symbols))))

(defun write-contribs ()
  (let ((failures nil))
    (with-open-file
        (stream (table-path "contribs.tsv")
                :direction
                :output
                :if-exists
                :supersede
                :if-does-not-exist
                :create
                :external-format
                :utf-8)
      (write-row stream '("name" "package" "status" "external-count" "reason"))
      (dolist (entry (append *sbcl-contribs* *additional-systems*))
        (destructuring-bind (name package-name) entry
          (cond
            ((not (contrib-in-scope-p name))
              (remove-stale-contrib-table name)
              (write-row stream
                         (list name
                               package-name
                               "out-of-scope"
                               "0"
                               "platform-not-in-scope")))
            ((not (try-require name))
              (push name failures)
              (remove-stale-contrib-table name)
              (write-row stream
                         (list name package-name "failed" "0" "load-failed")))
            (t
             (let ((package (find-package package-name)))
               (if package
                   (let ((count (write-contrib-table name package-name package)))
                     (write-row stream
                                (list name
                                      package-name
                                      "loaded"
                                      (princ-to-string count)
                                      "-")))
                   (progn
                     (push name failures)
                     (remove-stale-contrib-table name)
                     (write-row stream
                                (list name
                                      package-name
                                      "failed"
                                      "0"
                                      "package-not-found"))))))))))
    (nreverse failures)))

(defun main ()
  (unless (string= (lisp-implementation-version) "2.6.0")
    (error "This extractor requires SBCL 2.6.0"))
  (ensure-directories-exist (merge-pathnames ".keep" *symbols-directory*))
  (let ((references (read-internal-references))
        (counts nil)
        (contrib-failures nil))
    (dolist (package-name *public-packages*)
      (push (cons package-name (write-symbol-table package-name)) counts))
    (setf contrib-failures (write-contribs))
    (write-internal-references references)
    (format t "SBCL ~A~%" (lisp-implementation-version))
    (dolist (entry (nreverse counts))
      (format t "~A ~D~%" (car entry) (cdr entry)))
    (format t "internal-referenced ~D~%" (length references))
    (when contrib-failures
      (format *error-output*
              "In-scope contrib extraction failed: ~{~A~^, ~}~%"
              contrib-failures)
      (sb-ext:quit :unix-status 1))))

(main)

(sb-ext:quit :unix-status 0)

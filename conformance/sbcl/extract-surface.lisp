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
(defun write-platform-table (filename writer)
  (let* ((linux-p (x86-64-linux-p))
         (base-path (table-path filename))
         (target-path
           (if linux-p
               (merge-pathnames
                filename
                (merge-pathnames "linux-x86_64/" *symbols-directory*))
               base-path))
         (temporary-path nil)
         (stream nil)
         (published-p nil))
    (labels ((open-temporary ()
               (loop for suffix from 0
                     for pathname =
                       (make-pathname
                        :name (format nil ".~A.~D.tmp" (pathname-name target-path) suffix)
                        :type nil
                        :defaults target-path)
                     for candidate =
                       (open pathname
                             :direction :output
                             :if-exists nil
                             :if-does-not-exist :create
                             :external-format :utf-8)
                     when candidate
                       do (setf temporary-path pathname
                                stream candidate)
                          (return)))
             (files-identical-p (left right)
               (with-open-file (left-stream left
                                            :direction :input
                                            :element-type '(unsigned-byte 8))
                 (with-open-file (right-stream right
                                              :direction :input
                                              :element-type '(unsigned-byte 8))
                   (loop for left-byte = (read-byte left-stream nil nil)
                         for right-byte = (read-byte right-stream nil nil)
                         while (and left-byte right-byte (= left-byte right-byte))
                         finally (return (and (null left-byte)
                                              (null right-byte)))))))
      (ensure-directories-exist target-path)
      (open-temporary)
      (unwind-protect
           (let ((result (funcall writer stream)))
             (finish-output stream)
             (close stream)
             (setf stream nil)
             (require "asdf")
             (funcall
              (symbol-function
               (find-symbol "RENAME-FILE-OVERWRITING-TARGET" "UIOP"))
              temporary-path target-path)
             (setf published-p t)
             (when (and linux-p
                        (probe-file base-path)
                        (files-identical-p base-path target-path))
               (delete-file target-path))
             result)
        (progn
          (when stream
            (close stream :abort t))
          (when (and temporary-path
                     (not published-p)
                     (probe-file temporary-path))
            (delete-file temporary-path))))))))

(defun write-symbol-table (package-name)
  (let* ((package
           (or (find-package package-name)
               (error "Required package is missing")))
         (symbols (external-symbols package)))
    (write-platform-table
     (format nil "~A.tsv" (string-downcase package-name))
     (lambda (stream)
       (write-row stream *header*)
       (dolist (symbol symbols)
         (multiple-value-bind (found status)
             (find-symbol (symbol-name symbol) package)
           (unless (eq status :external)
             (error "External symbol disappeared during extraction"))
           (write-row stream (present-surface-fields package-name found status))))))
    (length symbols)))

(defun write-internal-references (references)
  (write-platform-table
   "internal-referenced.tsv"
   (lambda (stream)
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
                            (list libraries example-file exported))))))))

(defun x86-64-linux-p ()
  (and (member :x86-64 *features*) (member :linux *features*)))

(defun contrib-in-scope-p (name platform)
  (or (not (member name '("sb-simd" "sb-perf") :test #'string=))
      (string= platform "linux-x86_64")))

(defun try-require (name)
  (handler-case (progn
                  (require name)
                  t)
    (error ()
      nil)))

(defun remove-stale-contrib-table (name)
  (let* ((filename (format nil "contrib-~A.tsv" name))
         (path
           (if (x86-64-linux-p)
               (merge-pathnames
                filename
                (merge-pathnames "linux-x86_64/" *symbols-directory*))
               (table-path filename))))
    (when (probe-file path)
      (delete-file path))))

(defun write-contrib-table (name package-name package)
  (let ((symbols (external-symbols package)))
    (write-platform-table
     (format nil "contrib-~A.tsv" name)
     (lambda (stream)
       (write-row stream *header*)
       (dolist (symbol symbols)
         (multiple-value-bind (found status)
             (find-symbol (symbol-name symbol) package)
           (unless (eq status :external)
             (error "External symbol disappeared during extraction"))
           (write-row stream (present-surface-fields package-name found status))))))
    (length symbols)))

(defun write-contribs ()
  (let* ((entries (append *sbcl-contribs* *additional-systems*))
         (matrix-header
           '("name" "package"
             "macos-arm64-status" "macos-arm64-external-count" "macos-arm64-reason"
             "linux-x86_64-status" "linux-x86_64-external-count" "linux-x86_64-reason"))
         (legacy-header '("name" "package" "status" "external-count" "reason"))
         (platform
           (cond
             ((x86-64-linux-p) "linux-x86_64")
             ((and (member :darwin *features*)
                   (or (member :arm64 *features*)
                       (member :aarch64 *features*)))
              "macos-arm64")
             (t
              (error "Unsupported extraction platform"))))
         (base (if (string= platform "macos-arm64") 2 5)))
    (labels ((read-existing ()
               (let ((path (table-path "contribs.tsv"))
                     (rows (make-hash-table :test #'equal)))
                 (unless (probe-file path)
                   (error "Contrib metadata is missing"))
                 (with-open-file
                     (stream path
                             :direction :input
                             :external-format :utf-8)
                   (let* ((header-line (read-line stream nil nil))
                          (header (and header-line (split-tsv-line header-line))))
                     (unless (member header
                                     (list legacy-header matrix-header)
                                     :test #'equal)
                       (error "Invalid contrib metadata header"))
                     (loop for line = (read-line stream nil nil)
                           while line
                           do (let* ((fields (split-tsv-line line))
                                     (row
                                       (if (equal header legacy-header)
                                           (append (subseq fields 0 2)
                                                   (subseq fields 2)
                                                   (list "" "" ""))
                                           fields)))
                                (unless (= (length fields) (length header))
                                  (error "Malformed contrib metadata row"))
                                (unless (= (length row) (length matrix-header))
                                  (error "Malformed contrib metadata row"))
                                (unless (find (first row) entries
                                              :key #'first
                                              :test #'string=)
                                  (error "Unexpected contrib metadata row"))
                                (when (gethash (first row) rows)
                                  (error "Duplicate contrib metadata row"))
                                (loop for start in '(2 5)
                                      for values = (subseq row start (+ start 3))
                                      unless (or (every (lambda (value)
                                                          (zerop (length value)))
                                                        values)
                                                 (every (lambda (value)
                                                          (plusp (length value)))
                                                        values))
                                        do (error "Incomplete platform contrib metadata"))
                                (setf (gethash (first row) rows) row)))))
                 (dolist (entry entries)
                   (let ((row (gethash (first entry) rows)))
                     (unless row
                       (error "Missing contrib metadata row"))
                     (unless (string= (second row) (second entry))
                       (error "Contrib package does not match metadata"))))
                 (unless (= (hash-table-count rows) (length entries))
                   (error "Unexpected contrib metadata row count"))
                 rows))
             (set-platform-fields (row status count reason)
               (setf (nth base row) status
                     (nth (1+ base) row) count
                     (nth (+ base 2) row) reason))
             (remove-table-if-unused (name row)
               (declare (ignore row))
               (remove-stale-contrib-table name)))
      (let ((rows (read-existing))
            (failures nil))
        (dolist (entry entries)
          (destructuring-bind (name package-name) entry
            (let ((row (gethash name rows)))
              (cond
                ((not (contrib-in-scope-p name platform))
                 (set-platform-fields row "out-of-scope" "0" "platform-not-in-scope")
                 (remove-table-if-unused name row))
                ((not (try-require name))
                 (push name failures)
                 (set-platform-fields row "failed" "0" "load-failed")
                 (remove-table-if-unused name row))
                (t
                 (let ((package (find-package package-name)))
                   (if package
                       (let ((count (write-contrib-table name package-name package)))
                         (set-platform-fields row "loaded"
                                              (princ-to-string count)
                                              "-"))
                       (progn
                         (push name failures)
                         (set-platform-fields row "failed" "0" "package-not-found")
                         (remove-table-if-unused name row)))))))))
        (with-open-file
            (stream (table-path "contribs.tsv")
                    :direction :output
                    :if-exists :supersede
                    :if-does-not-exist :create
                    :external-format :utf-8)
          (write-row stream matrix-header)
          (dolist (entry entries)
            (write-row stream (gethash (first entry) rows))))
        (nreverse failures)))))

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

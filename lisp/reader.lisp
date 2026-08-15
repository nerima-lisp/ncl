(in-package #:ncl)

(defparameter +end-of-input+
  (gensym "END-OF-INPUT"))

(defun read-forms (source &optional (package *package*))
  (let ((*package*
         (etypecase package
           (package package)
           (symbol
            (or (find-package package) (error "Unknown package ~S." package)))
           (string
            (or (find-package package)
                (find-package (string-upcase package))
                (error "Unknown package ~S." package))))))
    (with-input-from-string (input source)
      (loop for form = (read input nil +end-of-input+)
            until (eq form +end-of-input+)
            collect form))))

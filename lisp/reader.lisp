(defun read-forms (source &optional (package *package*))
  (let ((*package*
         (typecase package
           (package package)
           (symbol
            (or (find-package package) (error "Unknown package ~S." package)))
           (string
            (or (find-package (string-upcase package))
                (error "Unknown package ~S." package)))
           (t
            (error "Package designator must be a package, symbol, or string: ~S."
                   package))))
        (end-of-input (gensym "END-OF-INPUT")))
    (with-input-from-string (input source)
      (loop for form = (read input nil end-of-input)
            until (eq form end-of-input)
            collect form))))

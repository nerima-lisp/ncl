(let* ((root
        (make-pathname
         :name nil
         :type nil
         :version nil
         :defaults (or *load-truename* *load-pathname* *default-pathname-defaults*)))
       (source-root (merge-pathnames "lisp/" root)))
  (dolist (file '("package.lisp"
                  "data.lisp"
                  "logic.lisp"
                  "reader.lisp"
                  "conditions.lisp"
                  "evaluator.lisp"
                  "lambda-list.lisp"
                  "standard.lisp"
                  "cli.lisp"))
    (load (merge-pathnames file source-root)))
  (let ((entry-point (find-symbol "RUN-COMMAND-LINE" "NCL")))
    (unless (and entry-point (fboundp entry-point))
      (error "NCL CLI entry point is not available after loading ncl."))
    (if (funcall entry-point)
        (sb-ext:exit :code 0)
        (sb-ext:exit :code 1))))

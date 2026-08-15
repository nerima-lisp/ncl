(let* ((root
        (make-pathname
         :name nil
         :type nil
         :version nil
         :defaults (or *load-truename* *load-pathname* *default-pathname-defaults*)))
       (source-root (merge-pathnames "lisp/" root))
       (test-root (merge-pathnames "test/" root)))
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
  (dolist (file '("package.lisp" "support.lisp" "core.lisp"))
    (load (merge-pathnames file test-root))))

(finish-output)

(require :asdf)

(let* ((root
        (make-pathname
         :name nil
         :type nil
         :version nil
         :defaults (or *load-truename* *load-pathname* *default-pathname-defaults*)))
       (system-file (merge-pathnames "ncl.asd" root)))
  (load system-file)
  (asdf:load-system "ncl")
  (let ((entry-point (find-symbol "RUN-COMMAND-LINE" "NCL")))
    (unless (and entry-point (fboundp entry-point))
      (error "NCL CLI entry point is not available after loading ncl."))
    (if (funcall entry-point)
        (sb-ext:exit :code 0)
        (sb-ext:exit :code 1))))

(require :asdf)

(let* ((root
        (make-pathname
         :name nil
         :type nil
         :version nil
         :defaults (or *load-truename* *load-pathname* *default-pathname-defaults*)))
       (system-file (merge-pathnames "ncl.asd" root)))
  (load system-file)
  (asdf:load-system "ncl/test"))

(finish-output)

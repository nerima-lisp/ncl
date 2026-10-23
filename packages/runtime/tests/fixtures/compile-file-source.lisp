(eval-when (:compile-toplevel)
  (setq *compile-file-pathname-observed* (namestring *compile-file-pathname*)
        *compile-file-truename-observed* (namestring *compile-file-truename*)))

(setq *compile-file-load-result* "MINIMAL-LOAD-OK")

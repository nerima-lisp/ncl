(in-package #:ncl/test)

(defmacro expect-value (source expected)
  `(expect (ncl:evaluate-source ,source) :to-be ,expected))

(defmacro expect-form (source expected)
  `(expect (ncl:evaluate-source ,source) :to-equal ,expected))

(defmacro expect-condition (condition-type &body body)
  `(expect
    (handler-case (progn
                    ,@body
                    nil)
      (,condition-type ()
        t))
    :to-be
    t))

(defmacro expect-invalid-sources (&body sources)
  `(progn
     ,@(mapcar (lambda (source)
                 `(expect-condition ncl:invalid-form-error
                                    (ncl:evaluate-source ,source)))
               sources)))

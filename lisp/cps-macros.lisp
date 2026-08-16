(defmacro cps-sequence (&body producers)
  (cond
    ((null producers) '(cps-return nil))
    ((null (cdr producers)) (car producers))
    (t
     `(cps-bind ,(car producers)
                (lambda (ignored)
                  (declare (ignore ignored))
                  (cps-sequence ,@(cdr producers)))))))

(defmacro cps-let ((name producer) &body body)
  `(cps-bind ,producer
             (lambda (,name)
               (declare (ignorable ,name))
               (cps-sequence ,@body))))

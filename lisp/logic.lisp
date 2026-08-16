(defun cps-return (value)
  (lambda (success failure)
    (declare (ignore failure))
    (funcall success value)))

(defun cps-fail (condition)
  (lambda (success failure)
    (declare (ignore success))
    (funcall failure condition)))

(defun cps-bind (producer next)
  (lambda (success failure)
    (funcall producer
             (lambda (value)
               (funcall (funcall next value) success failure))
             failure)))

(defun cps-run (producer &key (on-success #'identity) (on-failure #'error))
  (funcall producer on-success on-failure))

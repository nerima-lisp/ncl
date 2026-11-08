(flet ((identity (value) value))
  (funcall (lambda (argument) (identity argument)) 7))

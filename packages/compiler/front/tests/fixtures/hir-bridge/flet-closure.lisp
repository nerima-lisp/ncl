(flet ((local-callable (value) value))
  (funcall (funcall (lambda () #'local-callable)) 7))

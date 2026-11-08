(let ((x 4))
  (flet ((inc (y) (+ y 1)))
    (funcall #'inc x)))

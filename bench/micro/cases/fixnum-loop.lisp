(defun micro-case (iterations)
  (let ((sum 0))
    (dotimes (index iterations sum)
      (incf sum index))))

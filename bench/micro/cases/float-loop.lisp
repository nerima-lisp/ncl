(defun micro-case (iterations)
  (let ((sum 0.0d0))
    (dotimes (index iterations (floor sum))
      (incf sum (* (float index 1.0d0) 0.5d0)))))

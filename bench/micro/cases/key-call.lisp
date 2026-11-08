(defun micro-key-helper (&key value increment)
  (+ value increment))

(defun micro-case (iterations)
  (let ((sum 0))
    (dotimes (index iterations sum)
      (incf sum (micro-key-helper :value index :increment 1)))))

(defun micro-case (iterations)
  (labels ((walk (remaining sum)
             (if (zerop remaining)
                 sum
                 (walk (1- remaining) (1+ sum)))))
    (walk iterations 0)))

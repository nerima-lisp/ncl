(defstruct micro-record left right)

(defun micro-case (iterations)
  (let ((record (make-micro-record :left 17 :right 23))
        (sum 0))
    (dotimes (index iterations sum)
      (declare (ignore index))
      (incf sum (+ (micro-record-left record) (micro-record-right record))))))

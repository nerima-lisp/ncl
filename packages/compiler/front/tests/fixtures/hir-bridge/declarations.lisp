(locally
  (declare (type (integer 0 10) x)
           (optimize (speed 2) (safety 0))
           (inline helper))
  (the (integer 0 10) x))

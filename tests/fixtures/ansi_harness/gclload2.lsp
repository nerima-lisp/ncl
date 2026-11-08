(progn
  (defgeneric ansi-randomized-method (value)
    (:method-combination randomized))
  (defmethod ansi-randomized-method 1 ((value t))
    :low)
  (defmethod ansi-randomized-method 2 ((value t))
    :high))

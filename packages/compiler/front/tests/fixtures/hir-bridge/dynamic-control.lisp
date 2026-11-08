(progn
  (catch :tag (throw :tag 5))
  (progv '(special) '(5) special)
  (multiple-value-call #'list (values 1 2)))

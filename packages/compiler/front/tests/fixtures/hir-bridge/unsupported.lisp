(macrolet ((m (x) (quote x)))
  (m 1))
(symbol-macrolet ((x y))
  x)

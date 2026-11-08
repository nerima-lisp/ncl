(block done
  (unwind-protect
      (return-from done (multiple-value-prog1 (values 1 2) 3))
    4))

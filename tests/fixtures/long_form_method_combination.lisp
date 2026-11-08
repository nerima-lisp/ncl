(progn
  ;; CLHS 7.6.6: qualifier patterns select methods by exact qualifier-list equality.
  (define-method-combination ncl-ansiload-patterns ()
    ((audit-methods (:audit))
     (plain-methods ()))
    (assert (= (length audit-methods) 1))
    (assert (= (length plain-methods) 1))
    (assert (eq (first (method-qualifiers (first audit-methods))) :audit))
    (assert (null (method-qualifiers (first plain-methods))))
    `(loop
       (catch (quote done)
         (return
           (rcase
             (1 (call-method ,(first audit-methods)))
             (0 (call-method ,(first plain-methods))))))))
  (defgeneric ncl-ansiload-pattern-dispatch (value)
    (:method-combination ncl-ansiload-patterns))
  (defmethod ncl-ansiload-pattern-dispatch :audit ((value integer)) :audit)
  (defmethod ncl-ansiload-pattern-dispatch ((value integer)) :plain)
  (unless (eq (ncl-ansiload-pattern-dispatch 5) :audit)
    (error "literal qualifier pattern selected the wrong method group"))
  t)

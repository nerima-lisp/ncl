(progn
  (defun nested-native-gc ()
    (funcall (intern "GC" (find-package "NCL-GC"))))
  (defun parent-only-function-root ()
    (let ((function (lambda () 42)))
      (nested-native-gc)
      (funcall function)))
  (parent-only-function-root))

(in-package #:ncl)

(defparameter *version*
  "0.1.0")

(defun print-result (value stream)
  (format stream "~S~%" value)
  value)

(defun report-condition (condition stream)
  (format stream "Error: ~A~%" condition)
  nil)

(defun read-file-string (pathname)
  (with-open-file (input pathname)
    (with-output-to-string (output)
      (loop for line = (read-line input nil nil)
            while line
            do (write-line line output)))))

(defun run-file (pathname stream)
  (print-result (evaluate-source (read-file-string pathname)) stream))

(defun repl-loop (&optional (input *standard-input*) (output *standard-output*))
  (loop (format output "ncl> ") (finish-output output) (let ((line
                                                              (read-line input
                                                                         nil
                                                                         nil)))
                                                         (unless line
                                                           (return t))
                                                         (handler-case (print-result
                                                                        (evaluate-source
                                                                         line)
                                                                        output)
                                                           (condition (condition)
                                                             (report-condition
                                                              condition
                                                              *error-output*))))))

(defun print-help (&optional (stream *standard-output*))
  (format stream
          "NCL Common Lisp~%~%Usage: ncl [--eval FORM | --file PATH | --repl | --help | --version]~%Without an option, NCL starts a REPL.~%")
  t)

(defun command-line-arguments ()
  (let* ((argv sb-ext:*posix-argv*)
         (script-option (position "--script" argv :test #'string=)))
    (cond
      ((and script-option (< (+ script-option 1) (length argv)))
       (nthcdr (+ script-option 2) argv))
      ((position "--" argv :test #'string=)
       (rest (member "--" argv :test #'string=)))
      (t (rest argv)))))

(defun run-command-line (&optional (arguments (command-line-arguments)))
  (handler-case (cond
                  ((null arguments) (repl-loop))
                  ((string= (first arguments) "--help") (print-help))
                  ((string= (first arguments) "--version")
                    (format t "ncl ~A~%" *version*)
                    t)
                  ((string= (first arguments) "--repl") (repl-loop))
                  ((string= (first arguments) "--eval")
                   (if (second arguments)
                       (print-result (evaluate-source (second arguments))
                                     *standard-output*)
                       (progn
                         (format *error-output* "--eval requires a form.~%")
                         nil)))
                  ((string= (first arguments) "--file")
                   (if (second arguments)
                       (run-file (second arguments) *standard-output*)
                       (progn
                         (format *error-output* "--file requires a path.~%")
                         nil)))
                  (t
                    (format *error-output*
                            "Unknown option ~S.~%"
                            (first arguments))
                    (print-help *error-output*)
                    nil))
    (condition (condition)
      (report-condition condition *error-output*))))

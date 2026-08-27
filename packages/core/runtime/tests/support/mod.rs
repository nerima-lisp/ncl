use ncl_runtime::{Runtime, RuntimeError, Value};

pub const MALFORMED_SPECIAL_FORMS: &[&str] = &[
    "(quote)",
    "(the integer)",
    "(load-time-value)",
    "(nth-value -1 1)",
    "(nth-value symbol 1)",
    "(eval-when)",
    "(eval-when 1 (+ 1 2))",
    "(restart-bind)",
    "(restart-bind (not-a-clause) 1)",
    "(with-simple-restart)",
    "(with-simple-restart (abort) 1)",
    "(block (not-a-name) 1)",
    "(return-from (not-a-name) 1)",
    "(with-simple-restart (1) 1)",
    "(restart-case 1 (1 () 2))",
    "(handler-case 1 (1 (condition) 2))",
    "(handler-bind ((1 (lambda (condition) nil))) 1)",
    "(catch)",
    "(throw 1)",
    "(go)",
    "(unwind-protect)",
    "(let ((1 2)) 3)",
    "(let)",
    "(let 1 2)",
    "(let ((x 1 2)) x)",
    "(flet)",
    "(flet 1 2)",
    "(flet ((f)) (f))",
    "(macrolet)",
    "(macrolet 1 2)",
    "(macrolet ((m)) (m))",
    "(symbol-macrolet)",
    "(symbol-macrolet 1 2)",
    "(symbol-macrolet ((x)) x)",
    "(dotimes)",
    "(dotimes (x))",
    "(dotimes (x 1 2 3))",
    "(dolist)",
    "(dolist (x))",
    "(dolist (x nil 1 2))",
    "(progv '(1) nil nil)",
];

pub const MALFORMED_GENERALIZED_ASSIGNMENT_FORMS: &[&str] = &[
    "(setf)",
    "(psetf value 1 value)",
    "(push 1 2)",
    "(pop 1)",
    "(pushnew 1 2)",
    "(pushnew 1 (list 1) :test #'eql :test-not #'eql)",
    "(setf (car 1) 2)",
    "(setf (car) 2)",
    "(setf (cdr 1) '(2))",
    "(setf (cdr) '(2))",
    "(setf (cdr (list 1)) 2)",
    "(setf (nth 0) 2)",
    "(setf (nth 0 1) 2)",
    "(setf (nth -1 (list 1)) 2)",
    "(setf (first nil) 2)",
    "(setf (rest nil) '(2))",
    "(setf (aref #(1) 2) 3)",
    "(setf (svref '(1) 0) 3)",
    "(setf (svref #(1) -1) 2)",
    "(setf (char \"a\" 2) #\\X)",
    "(setf (char 1 0) #\\X)",
    "(setf (char \"a\" 0) 1)",
    "(setf (elt #(1) 2) 3)",
    "(setf (elt \"abc\" -1) #\\X)",
    "(setf (elt 1 0) 2)",
    "(setf (subseq (list 1 2) 2 1) '(3))",
    "(setf (subseq (list 1 2) 0 2) 3)",
    "(setf (subseq \"abc\" 0) 1)",
    "(setf (subseq \"abc\" 2 1) \"x\")",
    "(setf (subseq 1 0) '(2))",
    "(setf (row-major-aref #(1) 2) 3)",
    "(setf (bit #(0) 2) 1)",
    "(setf (symbol-value 1) 2)",
    "(setf (get 1 :answer) 2)",
    "(setf (getf '( :answer) :answer) 2)",
    "(setf (gethash :key 1) 2)",
    "(setf (symbol-function 'car) 2)",
    "(setf (symbol-function) #'car)",
    "(setf (symbol-function 'car #'car) #'cdr)",
    "(setf (slot-value) 2)",
    "(setf (slot-value 1 'name) 2)",
    "(setf (get) 2)",
    "(setf (get 'answer) 2)",
    "(setf (getf) 2)",
    "(setf (getf (list :answer 1)) 2)",
    "(setf (gethash) 2)",
    "(setf (gethash :key) 2)",
    "(setf (aref #(1) 0 1) 2)",
    "(setf (aref (make-array '(2 2)) 2 0) 2)",
    "(setf (bit #(0) 0) 2)",
    "(setf (subseq \"abc\" 0 2) '(1 2))",
    "(setf (elt \"abc\" 0) 1)",
];

pub const MALFORMED_CHARACTER_FORMS: &[&str] = &[
    "(char)",
    "(char \"a\")",
    "(char \"a\" -1)",
    "(char \"a\" 1)",
    "(char 1 0)",
    "(schar \"a\" 1)",
    "(schar 1 0)",
    "(character)",
    "(character 1 2)",
    "(char-code 1)",
    "(char-int 1)",
    "(code-char \"65\")",
    "(digit-char)",
    "(digit-char 1 1)",
    "(digit-char 1 37)",
    "(digit-char 1.5)",
    "(digit-char-p)",
    "(digit-char-p 1)",
    "(digit-char-p #\\1 1)",
    "(name-char)",
];

pub trait MustExist<T> {
    fn must_exist(self) -> T;
}

impl<T, E: std::fmt::Debug> MustExist<T> for Result<T, E> {
    fn must_exist(self) -> T {
        match self {
            Ok(value) => value,
            Err(error) => panic!("expected success, got {error:?}"),
        }
    }
}

impl<T> MustExist<T> for Option<T> {
    fn must_exist(self) -> T {
        self.unwrap_or_else(|| panic!("expected a value, got None"))
    }
}

pub trait MustFail<E> {
    fn must_fail(self) -> E;
}

impl<T: std::fmt::Debug, E> MustFail<E> for Result<T, E> {
    fn must_fail(self) -> E {
        match self {
            Ok(value) => panic!("expected failure, got {value:?}"),
            Err(error) => error,
        }
    }
}

pub fn evaluate_with<F>(evaluator: F, source: &str) -> Value
where
    F: FnOnce(&Runtime, &str) -> Result<Vec<Value>, RuntimeError>,
{
    let mut values = match evaluator(&Runtime::new(), source) {
        Ok(values) => values,
        Err(error) => panic!("test source must evaluate successfully: {error}"),
    };
    values
        .pop()
        .unwrap_or_else(|| panic!("test source must return a value"))
}

pub fn assert_evaluates_to<F>(evaluator: F, source: &str, expected: &str)
where
    F: FnOnce(&Runtime, &str) -> Result<Vec<Value>, RuntimeError>,
{
    assert_eq!(evaluate_with(evaluator, source).to_string(), expected);
}

pub fn assert_value_cases<F>(evaluate: F, cases: &[(&str, &str)])
where
    F: Fn(&str) -> Value,
{
    for (source, expected) in cases {
        assert_eq!(evaluate(source).to_string(), *expected, "source: {source}");
    }
}

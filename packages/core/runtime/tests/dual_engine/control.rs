use ncl_runtime::Runtime;
use rstest::rstest;

use super::support::evaluate_with;
use super::EvalFn;

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn expands_basic_loop_iteration(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            r"(let ((value 0))
                 (loop
                   (incf value)
                   (when (= value 3) (return value))))"
        )
        .to_string(),
        "3"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn expands_loop_condition_clauses(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            r"(let ((value 0))
                 (loop while (< value 3) do (incf value))
                 value)"
        )
        .to_string(),
        "3"
    );
    assert_eq!(
        evaluate(
            r"(let ((value 0))
                 (loop until (= value 3) (incf value))
                 value)"
        )
        .to_string(),
        "3"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn expands_loop_repeat_clause(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            r"(let ((value 0))
                 (loop repeat (+ 1 2) do (incf value))
                 value)"
        )
        .to_string(),
        "3"
    );
    assert_eq!(
        evaluate(
            r"(let ((value 0))
                 (loop repeat 3 do (incf value) finally (+ value 10)))"
        )
        .to_string(),
        "13"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn expands_loop_with_clause(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(r"(loop with value = 2 and other = 3 do (+ value other))").to_string(),
        "5"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn expands_loop_collect_clause(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            r"(let ((value 0))
                 (loop repeat 3 collect (incf value)))"
        )
        .to_string(),
        "(1 2 3)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn expands_loop_nconc_clause(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(r"(loop for value in (list 1 2 3) nconc (list value (* value 10)))" )
            .to_string(),
        "(1 10 2 20 3 30)"
    );
    assert_eq!(
        evaluate(r"(loop for value in (list 1 2) nconc (list value) into result)")
        .to_string(),
        "(1 2)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn expands_loop_for_clause(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(r"(loop for value from 1 to 3 collect value)").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate(
            r"(let ((total 0))
                 (loop for value from 1 to 3 do (incf total value))
                 total)"
        )
        .to_string(),
        "6"
    );
    assert_eq!(
        evaluate(r"(loop for value in (list 1 2 3) sum value)").to_string(),
        "6"
    );
    assert_eq!(
        evaluate(r"(loop for value in (list 3 1 2) maximize value into result)").to_string(),
        "3"
    );
    assert_eq!(
        evaluate(r"(loop for value in (list 3 1 2) minimize value)").to_string(),
        "1"
    );
    assert_eq!(
        evaluate(r"(loop for value in (list 1 2 3) count (evenp value))").to_string(),
        "1"
    );
    assert_eq!(
        evaluate(r"(loop for value in (list 1 2 3) count (evenp value) into total)").to_string(),
        "1"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn expands_loop_for_then_clause(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(r"(loop for value = 1 then (+ value 1) repeat 3 collect value)").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate(r"(loop for value = 1 then (+ value 1) while (< value 4) collect value)")
            .to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate(r"(loop for value = 1 then (+ value 1) until (> value 3) collect value)")
            .to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate(r"(loop for tail on (list 1 2 3) collect (car tail))").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate(
            r"(loop for tail on (list 1 2 3 4) by (lambda (value) (cdr (cdr value))) collect (car tail))"
        )
        .to_string(),
        "(1 3)"
    );
    assert_eq!(
        evaluate(r"(loop for value across #(1 2 3) collect value)").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate(r"(loop for value across #(1 3 2) sum value)").to_string(),
        "6"
    );
    assert_eq!(
        evaluate(r"(loop for value across #(1 3 2) maximize value into result)").to_string(),
        "3"
    );
    assert_eq!(
        evaluate(r"(loop for value across #(1 2 3) count (evenp value))").to_string(),
        "1"
    );
    assert_eq!(
        evaluate(r"(loop for value in (list (list 1 2) (list 3)) append value)").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate(r"(loop for value in (list (list 1 2) (list 3)) append value into result)")
            .to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate(r"(loop for value across #((1 2) (3)) append value into result)").to_string(),
        "(1 2 3)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn expands_loop_for_in_clause(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(r"(loop for value in (list 1 2 3) collect value)").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate(
            r"(let ((total 0))
                 (loop for value in (list 1 2 3) do (incf total value))
                 total)"
        )
        .to_string(),
        "6"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn expands_loop_for_numeric_limit_clauses(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(r"(loop for value from 1 below 4 collect value)").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate(r"(loop for value from 3 downto 1 collect value)").to_string(),
        "(3 2 1)"
    );
    assert_eq!(
        evaluate(r"(loop for value from 3 above 1 collect value)").to_string(),
        "(3 2)"
    );
    assert_eq!(
        evaluate(r"(loop for value from 1 to 6 by 2 collect value)").to_string(),
        "(1 3 5)"
    );
    assert_eq!(
        evaluate(r"(loop for value from 6 downto 1 by 2 collect value)").to_string(),
        "(6 4 2)"
    );
    assert_eq!(
        evaluate(r"(loop for value from 1 to 3 sum value)").to_string(),
        "6"
    );
    assert_eq!(
        evaluate(r"(loop for value from 1 to 3 sum (* value 2) into total)").to_string(),
        "12"
    );
    assert_eq!(
        evaluate(r"(loop for value from 1 to 3 count (evenp value))").to_string(),
        "1"
    );
    assert_eq!(
        evaluate(r"(loop for value from 1 to 3 maximize value)").to_string(),
        "3"
    );
    assert_eq!(
        evaluate(r"(loop for value from 1 to 3 minimize value into total)").to_string(),
        "1"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn captures_an_active_tagbody_target_in_a_closure(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    let source = r"
            (let ((value 0))
              (tagbody
                start
                (setq value 1)
                (funcall (lambda () (go done)))
                (setq value 99)
                done)
              value)
        ";

    assert_eq!(evaluate(source).to_string(), "1");
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_destructuring_bind_lambda_list_parameters(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(destructuring-bind (&whole whole (first second) &optional third)
                   (list (list 1 2) 3)
                   (list whole first second third))"
        )
        .to_string(),
        "(((1 2) 3) 1 2 3)",
    );
    assert_eq!(
        evaluate(
            "(destructuring-bind (first &optional (second (+ first 1) second-p)
                                      &key (scale 2 scale-p)
                                      &allow-other-keys
                                      &aux (total (+ first second scale)))
                   (list 3 :scale 4 :ignored 9)
                   (list first second second-p scale scale-p total))",
        )
        .to_string(),
        "(3 4 NIL 4 T 11)",
    );
    assert_eq!(
        evaluate(
            "(destructuring-bind (first &optional (second (+ first 1) second-p)
                                      &key (scale 2 scale-p))
                   (list 3 5 :scale 6)
                   (list first second second-p scale scale-p))",
        )
        .to_string(),
        "(3 5 T 6 T)",
    );
    assert_eq!(
        evaluate(
            "(destructuring-bind (first &rest rest &aux (count (length rest)))
                   (list 3 4 5)
                   (list first rest count))",
        )
        .to_string(),
        "(3 (4 5) 2)",
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_destructuring_bind_with_nested_and_dotted_patterns(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(destructuring-bind (first (second third)) (list 1 (list 2 3))
                   (+ first (+ second third)))"
        )
        .to_string(),
        "6"
    );
    assert_eq!(
        evaluate(
            "(destructuring-bind (head . tail) (list 1 2 3)
                   (+ head (car tail)))"
        )
        .to_string(),
        "3"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_do_and_do_star_with_parallel_and_sequential_bindings(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(list
                   (let ((i 9))
                     (do ((i 1) (j i)) ((= i 1) j)))
                   (let ((i 9))
                     (do* ((i 1) (j i)) ((= i 1) j)))
                   (do ((i 0 (1+ i)) (j 0 i)) ((= i 3) j))
                   (do* ((i 0 (1+ i)) (j 0 i)) ((= i 3) j)))",
        )
        .to_string(),
        "(9 1 2 3)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_do_with_implicit_block_and_tagbody(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(do ((i 0 (1+ i)))
                     ((= i 3) -1)
                   (if (= i 2) (go finished) (go next))
                   finished
                   (return-from nil 42)
                   next)"
        )
        .to_string(),
        "42"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_prog_and_prog_star_with_parallel_and_sequential_bindings(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(list
                   (let ((i 9))
                     (prog ((i 1) (j i)) (return-from nil (list i j))))
                   (let ((i 9))
                     (prog* ((i 1) (j i)) (return-from nil (list i j))))
                   (prog () 42))",
        )
        .to_string(),
        "((1 9) (1 1) NIL)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_prog_with_implicit_block_and_tagbody(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(prog ((i 0))
                   start
                   (setq i (1+ i))
                   (if (= i 2) (return-from nil i) (go start)))",
        )
        .to_string(),
        "2"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_return_as_an_implicit_nil_block_exit(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(prog ((value 1))
                   (return (+ value 41))
                   (setq value 0))",
        )
        .to_string(),
        "42"
    );
    assert_eq!(evaluate("(prog () (return))").to_string(), "NIL");
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn supports_integer_and_keyword_tags(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    let source = r"
            (let ((count 0))
              (tagbody
                10
                (setq count (+ count 1))
                (if (= count 2) (go :done) (go 10))
                :done)
              count)
        ";

    assert_eq!(evaluate(source).to_string(), "2");
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn tagbody_returns_nil_and_does_not_evaluate_labels(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate("(list (tagbody start done) 42)").to_string(),
        "(NIL 42)"
    );
}

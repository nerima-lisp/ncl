use super::Runtime;
use ncl_object::{HeapRootedWord, Word};

#[test]
fn quoted_uninterned_symbols_are_uninterned_and_distinct() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));

    let first = runtime
        .eval("(quote #:alpha)")
        .unwrap_or_else(|error| panic!("quoted uninterned symbol: {error}"));
    let first = HeapRootedWord::new(&runtime.context, first)
        .unwrap_or_else(|error| panic!("first symbol root: {error}"));
    assert_eq!(
        ncl_object::symbol_package(&runtime.context, first.get()),
        Ok(Word::NIL)
    );
    let first_name = ncl_object::symbol_name(&runtime.context, first.get())
        .unwrap_or_else(|error| panic!("first symbol name: {error}"));
    assert_eq!(runtime.format_result(first_name), "\"ALPHA\"");

    let second = runtime
        .eval("(quote #:alpha)")
        .unwrap_or_else(|error| panic!("second quoted uninterned symbol: {error}"));
    let second = HeapRootedWord::new(&runtime.context, second)
        .unwrap_or_else(|error| panic!("second symbol root: {error}"));
    assert_ne!(first.get(), second.get());

    let same_form_distinct_literals = runtime
        .eval("(eq (quote #:alpha) (quote #:alpha))")
        .unwrap_or_else(|error| panic!("distinct quoted uninterned symbols: {error}"));
    assert_eq!(runtime.format_result(same_form_distinct_literals), "NIL");

    let same_reader_symbol = runtime
        .eval("(let ((symbols '(#1=#:alpha #1#))) (eq (car symbols) (cadr symbols)))")
        .unwrap_or_else(|error| panic!("shared quoted uninterned symbol: {error}"));
    assert_eq!(runtime.format_result(same_reader_symbol), "T");

    let shared_across_functions = runtime
        .eval(
            "(let ((left (lambda () (quote #1=#:alpha))) \
                   (right (lambda () (quote #1#)))) \
               (eq (funcall left) (funcall right)))",
        )
        .unwrap_or_else(|error| panic!("shared uninterned symbol in nested functions: {error}"));
    assert_eq!(runtime.format_result(shared_across_functions), "T");
}

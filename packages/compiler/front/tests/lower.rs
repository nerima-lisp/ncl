//! One test per lowering rule in `compiler-pipeline.md` ("front end の lowering
//! 規則"), each asserting that every generated function passes `ncl_ir::verify`.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "integration tests assert on results"
)]

#[path = "support/forms.rs"]
mod forms;

use forms::Fixture;

use ncl_compiler_front::lower_toplevel;
use ncl_ir::{Compare, Function, OpKind, Terminator, verify};
use ncl_object::Word;

/// Fail with the function's text form when it does not verify.
fn assert_verifies(function: &Function) {
    if let Err(errors) = verify(function) {
        panic!("{} does not verify: {errors:?}\n{function}", function.name);
    }
}

/// Whether any operation in the function matches the predicate.
fn any_op(function: &Function, predicate: impl Fn(&OpKind) -> bool) -> bool {
    function
        .blocks
        .iter()
        .any(|block| block.ops.iter().any(|op| predicate(&op.kind)))
}

/// Whether any terminator in the function matches the predicate.
fn any_terminator(function: &Function, predicate: impl Fn(&Terminator) -> bool) -> bool {
    function
        .blocks
        .iter()
        .any(|block| predicate(&block.terminator))
}

/// Rule: a variable captured and assigned by two closures is boxed in one cell.
#[test]
fn boxes_a_variable_captured_and_assigned_by_two_lambdas() {
    let mut fx = Fixture::new();
    let x = fx.user("X");
    let let_symbol = fx.cl("LET");
    let lambda_symbol = fx.cl("LAMBDA");
    let function_symbol = fx.cl("FUNCTION");
    let setq_symbol = fx.cl("SETQ");
    let funcall_symbol = fx.cl("FUNCALL");

    let binding = fx.list(&[x, Word::NIL]);
    let bindings = fx.list(&[binding]);
    let setq_one = fx.list(&[setq_symbol, x, Word::NIL]);
    let lambda_one = fx.list(&[lambda_symbol, Word::NIL, setq_one]);
    let designator_one = fx.list(&[function_symbol, lambda_one]);
    let call_one = fx.list(&[funcall_symbol, designator_one]);
    let setq_two = fx.list(&[setq_symbol, x, Word::NIL]);
    let lambda_two = fx.list(&[lambda_symbol, Word::NIL, setq_two]);
    let designator_two = fx.list(&[function_symbol, lambda_two]);
    let call_two = fx.list(&[funcall_symbol, designator_two]);
    let form = fx.list(&[let_symbol, bindings, call_one, call_two, x]);

    let expr = fx.expand(form).expect("expand");
    let lowered = lower_toplevel(&expr).expect("lower");

    assert_verifies(&lowered.entry);
    assert_eq!(lowered.nested.len(), 2, "one function per lambda");
    assert!(
        any_op(&lowered.entry, |kind| matches!(
            kind,
            OpKind::Alloc { words: 1 }
        )),
        "the shared variable is boxed in a one-word cell"
    );
    for nested in &lowered.nested {
        assert_verifies(nested);
        assert!(
            any_op(nested, |kind| matches!(kind, OpKind::Store { .. })),
            "each closure writes the shared cell"
        );
        assert!(
            any_op(nested, |kind| matches!(
                kind,
                OpKind::Convert {
                    op: ncl_ir::Convert::WordToAddress,
                    ..
                }
            )),
            "the captured cell arrives as a word and is used as an address"
        );
    }
}

/// Rule: a non-escaping `block`/`tagbody` lowers to `Jump`, not a handler region.
#[test]
fn lowers_a_non_escaping_block_and_tagbody_to_jump() {
    let mut fx = Fixture::new();
    let block_symbol = fx.cl("BLOCK");
    let return_symbol = fx.cl("RETURN-FROM");
    let tagbody_symbol = fx.cl("TAGBODY");
    let go_symbol = fx.cl("GO");
    let name = fx.user("B");

    let returned = fx.list(&[return_symbol, name, Word::NIL]);
    let block = fx.list(&[block_symbol, name, returned]);
    let expr = fx.expand(block).expect("expand block");
    let lowered = lower_toplevel(&expr).expect("lower block");
    assert_verifies(&lowered.entry);
    assert!(
        any_terminator(&lowered.entry, |term| matches!(
            term,
            Terminator::Jump { .. }
        )),
        "return-from becomes a Jump"
    );
    assert!(
        lowered.entry.handler_regions.is_empty(),
        "a non-escaping block needs no handler region"
    );

    let tag = fx.user("END");
    let go = fx.list(&[go_symbol, tag]);
    let tagbody = fx.list(&[tagbody_symbol, go, tag, Word::NIL]);
    let expr = fx.expand(tagbody).expect("expand tagbody");
    let lowered = lower_toplevel(&expr).expect("lower tagbody");
    assert_verifies(&lowered.entry);
    assert!(
        any_terminator(&lowered.entry, |term| matches!(
            term,
            Terminator::Jump { .. }
        )),
        "go becomes a Jump"
    );
    assert!(lowered.entry.handler_regions.is_empty());
}

/// Rule: multiple values go through the variadic adapter and `SetMultipleValues`.
#[test]
fn lowers_multiple_value_forms_through_the_adapter() {
    let mut fx = Fixture::new();
    let prog1_symbol = fx.cl("MULTIPLE-VALUE-PROG1");
    let call_symbol = fx.cl("MULTIPLE-VALUE-CALL");
    let function_symbol = fx.cl("FUNCTION");

    let first = fx.form("FOO", &[]);
    let rest = fx.form("BAR", &[]);
    let prog1 = fx.list(&[prog1_symbol, first, rest]);
    let expr = fx.expand(prog1).expect("expand multiple-value-prog1");
    let lowered = lower_toplevel(&expr).expect("lower multiple-value-prog1");
    assert_verifies(&lowered.entry);
    assert!(
        any_op(&lowered.entry, |kind| matches!(
            kind,
            OpKind::SetMultipleValues { .. }
        )),
        "the first form's values are recorded"
    );

    let list_name = fx.cl("LIST");
    let designator = fx.list(&[function_symbol, list_name]);
    let adapter = fx.list(&[call_symbol, designator, first, rest]);
    let expr = fx.expand(adapter).expect("expand multiple-value-call");
    let lowered = lower_toplevel(&expr).expect("lower multiple-value-call");
    assert_verifies(&lowered.entry);
    assert!(
        any_op(&lowered.entry, |kind| matches!(
            kind,
            OpKind::Builtin { name, .. } if name == "multiple-value-call"
        )),
        "multiple-value-call uses the variadic adapter builtin"
    );
}

/// Rule: `&optional` lowers to a `LoadArg` and `argc` comparison and default block.
#[test]
fn lowers_an_optional_parameter_with_a_default_block() {
    let mut fx = Fixture::new();
    let lambda_symbol = fx.cl("LAMBDA");
    let optional = fx.cl("&OPTIONAL");
    let parameter = fx.user("A");
    let defaulted = fx.user("B");
    let optional_parameter = fx.list(&[defaulted, Word::NIL]);
    let lambda_list = fx.list(&[parameter, optional, optional_parameter]);
    let lambda = fx.list(&[lambda_symbol, lambda_list, defaulted]);
    let call = fx.list(&[lambda, Word::NIL]);

    let expr = fx.expand(call).expect("expand");
    let lowered = lower_toplevel(&expr).expect("lower");

    assert_verifies(&lowered.entry);
    assert_eq!(lowered.nested.len(), 1, "the lambda is one function");
    let lambda_function = &lowered.nested[0];
    assert_verifies(lambda_function);
    assert_eq!(
        lambda_function.params.len(),
        3,
        "argc, the required parameter, and the optional parameter"
    );
    assert!(
        any_terminator(lambda_function, |term| matches!(
            term,
            Terminator::Branch { .. }
        )),
        "the optional parameter branches on whether it was supplied"
    );
    assert!(
        any_op(lambda_function, |kind| matches!(
            kind,
            OpKind::Compare {
                op: Compare::Ge,
                ..
            }
        )),
        "the branch compares argc against the parameter position"
    );
    assert!(
        any_op(lambda_function, |kind| matches!(
            kind,
            OpKind::LoadArg { index: 2 }
        )),
        "the supplied value is loaded from its argument slot"
    );
}

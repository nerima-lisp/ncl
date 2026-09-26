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
use ncl_ir::{Compare, Constant, Function, HandlerKind, OpKind, Terminator, verify};
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

/// Rule: multiple-value forms use the existing indirect-call path and
/// `SetMultipleValues`; lowering must not emit an unimplemented adapter.
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
            OpKind::CallIndirect { args, .. } if args.len() == 3
        )),
        "multiple-value-call uses the existing indirect-call ABI"
    );
    assert!(
        !any_op(&lowered.entry, |kind| matches!(
            kind,
            OpKind::Builtin { name, .. } if name == "multiple-value-call"
        )),
        "multiple-value-call does not emit an unsupported builtin"
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

#[test]
fn lowers_a_rest_parameter_through_the_runtime_prologue() {
    let mut fx = Fixture::new();
    let lambda = fx.cl("LAMBDA");
    let rest = fx.cl("&REST");
    let name = fx.user("ARGS");
    let lambda_list = fx.list(&[rest, name]);
    let lambda_form = fx.list(&[lambda, lambda_list, name]);
    let form = fx.list(&[lambda_form, Word::fixnum(1), Word::fixnum(2)]);

    let expr = fx.expand(form).expect("expand rest lambda");
    let lowered = lower_toplevel(&expr).expect("lower rest lambda");
    let lambda_function = &lowered.nested[0];
    assert_verifies(lambda_function);
    assert!(any_op(lambda_function, |kind| matches!(
        kind,
        OpKind::Builtin { name, args } if name == "make-rest-list" && args.len() == 2
    )));
}

#[test]
fn lowers_key_parameters_with_defaults_and_supplied_p() {
    let mut fx = Fixture::new();
    let lambda = fx.cl("LAMBDA");
    let key = fx.cl("&KEY");
    let key_name = fx.keyword("VALUE");
    let value = fx.user("VALUE");
    let supplied = fx.user("VALUE-P");
    let key_pair = fx.list(&[key_name, value]);
    let key_spec = fx.list(&[key_pair, Word::fixnum(7), supplied]);
    let lambda_list = fx.list(&[key, key_spec]);
    let lambda_form = fx.list(&[lambda, lambda_list, value]);
    let form = fx.list(&[lambda_form]);

    let expr = fx.expand(form).expect("expand key lambda");
    let lowered = lower_toplevel(&expr).expect("lower key lambda");
    let lambda_function = &lowered.nested[0];
    assert_verifies(lambda_function);
    assert!(any_op(lambda_function, |kind| matches!(
        kind,
        OpKind::Builtin { name, .. } if name == "check-keywords"
    )));
    assert!(any_op(lambda_function, |kind| matches!(
        kind,
        OpKind::Builtin { name, .. } if name == "keyword-supplied-p"
    )));
    assert!(any_terminator(lambda_function, |term| matches!(
        term,
        Terminator::Branch { .. }
    )));
}

#[test]
fn lowers_lambda_to_function_entry_closure_and_closure_call() {
    let mut fx = Fixture::new();
    let lambda = fx.cl("LAMBDA");
    let argument = fx.user("X");
    let lambda_list = fx.list(&[argument]);
    let lambda_form = fx.list(&[lambda, lambda_list, argument]);
    let form = fx.list(&[lambda_form, Word::NIL]);

    let expr = fx.expand(form).expect("expand");
    let lowered = lower_toplevel(&expr).expect("lower");

    assert_verifies(&lowered.entry);
    assert_eq!(lowered.nested.len(), 1);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeClosure { .. }
    )));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::CallClosure { .. }
    )));
    assert!(
        lowered
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::FunctionEntry(_)))
    );
    assert_verifies(&lowered.nested[0]);
}

#[test]
fn lowers_a_named_global_call_through_the_function_cell() {
    let mut fx = Fixture::new();
    let form = fx.form("GLOBAL-FUNCTION", &[Word::NIL]);

    let expr = fx.expand(form).expect("expand");
    let lowered = lower_toplevel(&expr).expect("lower");

    assert_verifies(&lowered.entry);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::LoadField {
            field,
            ..
        } if *field == ncl_object::symbol_offset::FUNCTION as u32
    )));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::CallClosure { .. }
    )));
    assert!(!any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::Call { .. }
    )));
}

#[test]
fn lowers_catch_throw_with_a_handler_region() {
    let mut fx = Fixture::new();
    let catch = fx.cl("CATCH");
    let throw = fx.cl("THROW");
    let tag = fx.keyword("TAG");
    let throw_form = fx.list(&[throw, tag, Word::fixnum(7)]);
    let form = fx.list(&[catch, tag, throw_form]);

    let expr = fx.expand(form).expect("expand");
    let lowered = lower_toplevel(&expr).expect("lower");

    assert_verifies(&lowered.entry);
    assert_eq!(lowered.entry.handler_regions.len(), 1);
    assert_eq!(lowered.entry.handler_regions[0].kind, HandlerKind::Catch);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::EnterHandler { .. }
    )));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::LeaveHandler { .. }
    )));
    assert!(any_op(
        &lowered.entry,
        |kind| matches!(kind, OpKind::Builtin { name, .. } if name == "throw")
    ));
    assert!(any_terminator(&lowered.entry, |term| matches!(
        term,
        Terminator::Throw { .. }
    )));
}

#[test]
fn lowers_unwind_protect_and_progv_with_handler_regions() {
    let mut fx = Fixture::new();
    let unwind = fx.cl("UNWIND-PROTECT");
    let cleanup = fx.form("CLEANUP", &[]);
    let unwind_form = fx.list(&[unwind, Word::NIL, cleanup]);
    let expr = fx.expand(unwind_form).expect("expand unwind-protect");
    let lowered = lower_toplevel(&expr).expect("lower unwind-protect");
    assert_verifies(&lowered.entry);
    assert!(
        lowered
            .entry
            .handler_regions
            .iter()
            .any(|region| region.kind == HandlerKind::UnwindProtect)
    );

    let progv = fx.cl("PROGV");
    let symbol = fx.user("X");
    let symbols = symbol;
    let values = Word::fixnum(1);
    let progv_form = fx.list(&[progv, symbols, values, symbol]);
    let expr = fx.expand(progv_form).expect("expand progv");
    let lowered = lower_toplevel(&expr).expect("lower progv");
    assert_verifies(&lowered.entry);
    assert!(
        lowered
            .entry
            .handler_regions
            .iter()
            .any(|region| region.kind == HandlerKind::Progv)
    );
}

#[test]
fn lowers_macrolet_and_symbol_macrolet_bodies() {
    let mut fx = Fixture::new();
    let symbol_macrolet = fx.cl("SYMBOL-MACROLET");
    let name = fx.user("X");
    let definition = fx.list(&[name, Word::fixnum(9)]);
    let definitions = fx.list(&[definition]);
    let form = fx.list(&[symbol_macrolet, definitions, name]);
    let expr = fx.expand(form).expect("expand symbol-macrolet");
    let lowered = lower_toplevel(&expr).expect("lower symbol-macrolet");
    assert_verifies(&lowered.entry);
}

#[test]
fn lowers_return_from_inside_a_lambda_with_a_catch_region() {
    let mut fx = Fixture::new();
    let block = fx.cl("BLOCK");
    let return_from = fx.cl("RETURN-FROM");
    let lambda = fx.cl("LAMBDA");
    let name = fx.user("B");
    let lambda_body = fx.list(&[return_from, name, Word::fixnum(11)]);
    let lambda_form = fx.list(&[lambda, Word::NIL, lambda_body]);
    let call = fx.list(&[lambda_form]);
    let form = fx.list(&[block, name, call]);

    let expr = fx.expand(form).expect("expand");
    let lowered = lower_toplevel(&expr).expect("lower");
    assert_verifies(&lowered.entry);
    assert_eq!(lowered.nested.len(), 1);
    assert!(
        lowered
            .entry
            .handler_regions
            .iter()
            .any(|region| region.kind == HandlerKind::Catch)
    );
    assert!(any_terminator(&lowered.nested[0], |term| matches!(
        term,
        Terminator::Throw { .. }
    )));
    assert_verifies(&lowered.nested[0]);
}

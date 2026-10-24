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

use ncl_compiler_front::{lower_toplevel, lower_toplevel_for_eval};
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

#[test]
fn eval_lowering_publishes_single_value_defaults_in_nested_functions() {
    let mut fx = Fixture::new();
    let lambda_symbol = fx.cl("LAMBDA");
    let lambda = fx.list(&[lambda_symbol, Word::NIL, Word::fixnum(17)]);
    let expr = fx.expand(lambda).expect("expand lambda");

    let ordinary = lower_toplevel(&expr).expect("ordinary lower");
    let eval = lower_toplevel_for_eval(&expr).expect("EVAL lower");

    let is_value_default =
        |kind: &OpKind| matches!(kind, OpKind::SetMultipleValues { values } if values.len() == 1);
    assert!(
        !any_op(&ordinary.entry, is_value_default),
        "ordinary lowering does not publish EVAL return state"
    );
    assert!(any_op(&eval.entry, is_value_default));
    assert_eq!(eval.nested.len(), 1);
    assert!(any_op(&eval.nested[0], is_value_default));
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
            OpKind::MakeValueCell { .. }
        )),
        "the shared variable is boxed in a one-word cell"
    );
    for nested in &lowered.nested {
        assert_verifies(nested);
        assert!(
            any_op(nested, |kind| matches!(
                kind,
                OpKind::StoreField { field: 0, .. }
            )),
            "each closure writes the shared cell"
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

/// Rule: a LET initializer that exits an enclosing block skips the LET body.
#[test]
fn let_initializer_return_from_escaping_block_verifies() {
    let mut fx = Fixture::new();
    let failure = fx.user("FAILURE");
    let tag = fx.user("TAG");
    let handler = fx.user("HANDLER");
    let value = fx.user("VALUE");

    let assign_failure = fx.form("SETQ", &[failure, Word::NIL]);
    let handler_return = fx.form("RETURN-FROM", &[tag, assign_failure]);
    let lambda = fx.form("LAMBDA", &[Word::NIL, handler_return]);
    let handler_function = fx.form("FUNCTION", &[lambda]);
    let handler_binding = fx.list(&[handler, handler_function]);
    let handler_bindings = fx.list(&[handler_binding]);

    let return_value = fx.form("RETURN-FROM", &[tag, Word::NIL]);
    let value_binding = fx.list(&[value, return_value]);
    let value_bindings = fx.list(&[value_binding]);
    let body = fx.form("PROGN", &[Word::NIL, value]);
    let value_let = fx.form("LET", &[value_bindings, body]);
    let handler_let = fx.form("LET", &[handler_bindings, value_let]);
    let block = fx.form("BLOCK", &[tag, handler_let]);
    let failure_binding = fx.list(&[failure, Word::NIL]);
    let failure_bindings = fx.list(&[failure_binding]);
    let form = fx.form("LET", &[failure_bindings, block]);

    let expr = fx.expand(form).expect("expand");
    let lowered = lower_toplevel(&expr).expect("lower");

    assert_eq!(lowered.nested.len(), 1, "the handler lambda is lowered");
    assert!(!lowered.entry.handler_regions.is_empty());
    assert_verifies(&lowered.entry);
    for nested in &lowered.nested {
        assert_verifies(nested);
    }
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
            OpKind::CallClosure { args, .. } if args.len() == 4
        )),
        "multiple-value-call uses the list adapter closure ABI"
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
        OpKind::Builtin { name, args, .. } if name == "make-rest-list" && args.len() == 2
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
fn lowers_lambda_value_in_let_body() {
    let mut fx = Fixture::new();
    let let_symbol = fx.cl("LET");
    let lambda_symbol = fx.cl("LAMBDA");
    let funcall_symbol = fx.cl("FUNCALL");
    let y = fx.user("Y");
    let x = fx.user("X");
    let plus = fx.cl("+");

    let binding = fx.list(&[y, Word::fixnum(5)]);
    let lambda_list = fx.list(&[x]);
    let sum = fx.list(&[plus, x, y]);
    let lambda = fx.list(&[lambda_symbol, lambda_list, sum]);
    let bindings = fx.list(&[binding]);
    let let_form = fx.list(&[let_symbol, bindings, lambda]);
    let form = fx.list(&[funcall_symbol, let_form, Word::fixnum(10)]);

    let expr = fx.expand(form).expect("expand lambda value in let");
    let lowered = lower_toplevel(&expr).expect("lower lambda value in let");

    assert_verifies(&lowered.entry);
    assert_eq!(lowered.nested.len(), 1);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeClosure { captures, .. } if captures.len() == 1
    )));
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
        } if *field == u32::try_from(ncl_object::symbol_offset::FUNCTION).unwrap_or_default()
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
fn lowers_unary_increment_and_decrement_as_builtins() {
    let mut fx = Fixture::new();

    for name in ["1+", "1-"] {
        let function = fx.cl(name);
        let form = fx.list(&[function, Word::fixnum(4)]);
        let expr = fx.expand(form).expect("expand unary arithmetic");
        let lowered = lower_toplevel(&expr).expect("lower unary arithmetic");

        assert_verifies(&lowered.entry);
        assert!(
            any_op(&lowered.entry, |kind| matches!(
                kind,
                OpKind::Builtin {
                    name: builtin,
                    args,
                    ..
                }
                    if builtin == name && args.len() == 1
            )),
            "{name} should lower to a one-argument builtin"
        );
    }
}

#[test]
fn lowers_local_unary_arithmetic_names_as_functions() {
    let mut fx = Fixture::new();

    for name in ["1+", "1-"] {
        let function = fx.cl(name);
        let argument = fx.user("X");
        let lambda_list = fx.list(&[argument]);
        let definition = fx.list(&[function, lambda_list, argument]);
        let definitions = fx.list(&[definition]);
        let call = fx.list(&[function, Word::fixnum(4)]);
        let form = fx.form("FLET", &[definitions, call]);
        let expr = fx.expand(form).expect("expand local unary function");
        let lowered = lower_toplevel(&expr).expect("lower local unary function");

        assert_verifies(&lowered.entry);
        assert!(!any_op(&lowered.entry, |kind| matches!(
            kind,
            OpKind::Builtin { name: builtin, .. } if builtin == name
        )));
        assert!(any_op(&lowered.entry, |kind| matches!(
            kind,
            OpKind::CallClosure { .. }
        )));
    }
}

#[test]
fn lowers_non_common_lisp_unary_arithmetic_names_as_functions() {
    let mut fx = Fixture::new();

    for name in ["1+", "1-"] {
        let function = fx.intern("NCL-TEST", name);
        let form = fx.list(&[function, Word::fixnum(4)]);
        let expr = fx
            .expand(form)
            .expect("expand package-local unary function");
        let lowered = lower_toplevel(&expr).expect("lower package-local unary function");

        assert_verifies(&lowered.entry);
        assert!(!any_op(&lowered.entry, |kind| matches!(
            kind,
            OpKind::Builtin { name: builtin, .. } if builtin == name
        )));
        assert!(any_op(&lowered.entry, |kind| matches!(
            kind,
            OpKind::CallClosure { .. }
        )));
    }
}

#[test]
fn lexical_type_and_the_forms_emit_word_type_hints() {
    let mut fx = Fixture::new();
    let x = fx.user("X");
    let y = fx.user("Y");
    let binding = fx.list(&[x, y]);
    let bindings = fx.list(&[binding]);
    let type_declaration = fx.cl("TYPE");
    let type_specifier = fx.cl("FIXNUM");
    let declaration = fx.list(&[type_declaration, type_specifier, x]);
    let declare = fx.form("DECLARE", &[declaration]);
    let sum = fx.form("+", &[x, x]);
    let form = fx.form("LET", &[bindings, declare, sum]);
    let lowered = lower_toplevel(&fx.expand(form).expect("expand")).expect("lower");

    assert_verifies(&lowered.entry);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::TypeHint {
            ty: ncl_ir::ScalarTypeHint::Fixnum,
            ..
        }
    )));

    let integer = fx.cl("INTEGER");
    let the = fx.form("THE", &[integer, y]);
    let lowered = lower_toplevel(&fx.expand(the).expect("expand THE")).expect("lower THE");
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::TypeHint {
            ty: ncl_ir::ScalarTypeHint::Integer,
            ..
        }
    )));
}

#[test]
fn inner_let_binding_shadows_outer_type_declaration() {
    let mut fx = Fixture::new();
    let x = fx.user("X");
    let y = fx.user("Y");
    let z = fx.user("Z");
    let outer_binding = fx.list(&[x, y]);
    let outer_bindings = fx.list(&[outer_binding]);
    let type_declaration = fx.cl("TYPE");
    let fixnum = fx.cl("FIXNUM");
    let declaration = fx.list(&[type_declaration, fixnum, x]);
    let declare = fx.form("DECLARE", &[declaration]);
    let inner_binding = fx.list(&[x, z]);
    let inner_bindings = fx.list(&[inner_binding]);
    let sum = fx.form("+", &[x, x]);
    let inner = fx.form("LET", &[inner_bindings, sum]);
    let outer = fx.form("LET", &[outer_bindings, declare, inner]);
    let lowered = lower_toplevel(&fx.expand(outer).expect("expand")).expect("lower");

    assert_verifies(&lowered.entry);
    assert!(!any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::TypeHint { .. }
    )));
}

#[test]
fn let_star_binding_shadows_outer_type_during_later_initializers() {
    let mut fx = Fixture::new();
    let x = fx.user("X");
    let z = fx.user("Z");
    let outer_binding = fx.list(&[x, Word::fixnum(1)]);
    let outer_bindings = fx.list(&[outer_binding]);
    let type_symbol = fx.cl("TYPE");
    let fixnum = fx.cl("FIXNUM");
    let declaration = fx.list(&[type_symbol, fixnum, x]);
    let declare = fx.form("DECLARE", &[declaration]);
    let first = fx.list(&[x, Word::fixnum(2)]);
    let second = fx.list(&[z, x]);
    let bindings = fx.list(&[first, second]);
    let inner = fx.form("LET*", &[bindings, z]);
    let outer = fx.form("LET", &[outer_bindings, declare, inner]);
    let lowered = lower_toplevel(&fx.expand(outer).expect("expand")).expect("lower");

    assert_verifies(&lowered.entry);
    assert!(!any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::TypeHint { .. }
    )));
}

#[test]
fn lambda_parameters_shadow_outer_type_declarations() {
    let mut fx = Fixture::new();
    let x = fx.user("X");
    let outer_binding = fx.list(&[x, Word::fixnum(1)]);
    let outer_bindings = fx.list(&[outer_binding]);
    let type_symbol = fx.cl("TYPE");
    let fixnum = fx.cl("FIXNUM");
    let declaration = fx.list(&[type_symbol, fixnum, x]);
    let declare = fx.form("DECLARE", &[declaration]);
    let lambda_symbol = fx.cl("LAMBDA");
    let lambda_list = fx.list(&[x]);
    let lambda = fx.list(&[lambda_symbol, lambda_list, x]);
    let call = fx.list(&[lambda, Word::fixnum(2)]);
    let outer = fx.form("LET", &[outer_bindings, declare, call]);
    let lowered = lower_toplevel(&fx.expand(outer).expect("expand")).expect("lower");

    assert_eq!(lowered.nested.len(), 1);
    assert_verifies(&lowered.nested[0]);
    assert!(!any_op(&lowered.nested[0], |kind| matches!(
        kind,
        OpKind::TypeHint { .. }
    )));
}

#[test]
fn local_function_parameters_shadow_outer_type_declarations() {
    let mut fx = Fixture::new();
    let x = fx.user("X");
    let f = fx.user("F");
    let outer_binding = fx.list(&[x, Word::fixnum(1)]);
    let outer_bindings = fx.list(&[outer_binding]);
    let type_symbol = fx.cl("TYPE");
    let fixnum = fx.cl("FIXNUM");
    let declaration = fx.list(&[type_symbol, fixnum, x]);
    let declare = fx.form("DECLARE", &[declaration]);
    let function_lambda_list = fx.list(&[x]);
    let definition = fx.list(&[f, function_lambda_list, x]);
    let definitions = fx.list(&[definition]);
    let call = fx.list(&[f, Word::fixnum(2)]);
    let flet = fx.form("FLET", &[definitions, call]);
    let outer = fx.form("LET", &[outer_bindings, declare, flet]);
    let lowered = lower_toplevel(&fx.expand(outer).expect("expand")).expect("lower");

    assert_eq!(lowered.nested.len(), 1);
    assert_verifies(&lowered.nested[0]);
    assert!(!any_op(&lowered.nested[0], |kind| matches!(
        kind,
        OpKind::TypeHint { .. }
    )));
}

#[test]
fn ftype_hints_direct_named_call_results_and_speed_zero_is_scoped() {
    let mut fx = Fixture::new();
    let function_type_name = fx.cl("FUNCTION");
    let fixnum = fx.cl("FIXNUM");
    let function_type = fx.list(&[function_type_name, Word::NIL, fixnum]);
    let ftype_name = fx.cl("FTYPE");
    let function_name = fx.user("F");
    let ftype = fx.list(&[ftype_name, function_type, function_name]);
    let declare_ftype = fx.form("DECLARE", &[ftype]);
    let call = fx.list(&[function_name]);
    let locally = fx.form("LOCALLY", &[declare_ftype, call]);
    let lowered = lower_toplevel(&fx.expand(locally).expect("expand FTYPE")).expect("lower FTYPE");
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::TypeHint {
            ty: ncl_ir::ScalarTypeHint::Fixnum,
            ..
        }
    )));

    let optimize = fx.cl("OPTIMIZE");
    let speed = fx.cl("SPEED");
    let speed_zero_quality = fx.list(&[speed, Word::fixnum(0)]);
    let speed_zero = fx.list(&[optimize, speed_zero_quality]);
    let declare_slow = fx.form("DECLARE", &[speed_zero]);
    let slow_call = fx.form("+", &[Word::fixnum(1), Word::fixnum(2)]);
    let speed_three_quality = fx.list(&[speed, Word::fixnum(3)]);
    let speed_three = fx.list(&[optimize, speed_three_quality]);
    let declare_fast = fx.form("DECLARE", &[speed_three]);
    let fast_call = fx.form("+", &[Word::fixnum(3), Word::fixnum(4)]);
    let nested = fx.form("LOCALLY", &[declare_fast, fast_call]);
    let form = fx.form("LOCALLY", &[declare_slow, slow_call, nested]);
    let lowered =
        lower_toplevel(&fx.expand(form).expect("expand optimize")).expect("lower optimize");
    let disabled = lowered.entry.blocks[0]
        .ops
        .iter()
        .filter_map(|op| match &op.kind {
            OpKind::Builtin {
                specialization_disabled,
                ..
            } => Some(*specialization_disabled),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(disabled, vec![true, false]);
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

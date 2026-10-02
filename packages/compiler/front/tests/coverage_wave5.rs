//! Fifth-wave coverage for the recursive analysis walkers and lower adapters.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use ncl_compiler_front::{
    AuxParam, Expr, FunctionDesignator, KeyParam, LambdaExpr, LambdaList, LetBinding, Literal,
    LocalFunction, Operator, OptionalParam, ParamName, SymbolRef, lower_toplevel,
};
use ncl_ir::{Function, HandlerKind, OpKind, Terminator, verify};

fn symbol(name: &str) -> SymbolRef {
    SymbolRef::interned("COMMON-LISP-USER", name)
}

fn assert_verifies(function: &Function) {
    if let Err(errors) = verify(function) {
        panic!("{} does not verify: {errors:?}\n{function}", function.name);
    }
}

fn any_op(function: &Function, predicate: impl Fn(&OpKind) -> bool) -> bool {
    function
        .blocks
        .iter()
        .any(|block| block.ops.iter().any(|op| predicate(&op.kind)))
}

fn any_terminator(function: &Function, predicate: impl Fn(&Terminator) -> bool) -> bool {
    function
        .blocks
        .iter()
        .any(|block| predicate(&block.terminator))
}

const fn lambda(body: Vec<Expr>, lambda_list: LambdaList) -> LambdaExpr {
    LambdaExpr {
        lambda_list,
        declarations: Vec::new(),
        docstring: None,
        body,
    }
}

#[test]
fn analysis_walks_return_through_flet_labels_and_nested_expression_forms() {
    let exit = symbol("EXIT");
    let return_form = || Expr::ReturnFrom {
        name: exit.clone(),
        value: Some(Box::new(Expr::Constant(Literal::fixnum(3)))),
    };
    let local = LocalFunction {
        name: symbol("LOCAL"),
        lambda: lambda(
            vec![Expr::Progn(vec![Expr::If {
                test: Box::new(Expr::Constant(Literal::T)),
                then: Box::new(return_form()),
                otherwise: Some(Box::new(Expr::Constant(Literal::Nil))),
            }])],
            LambdaList::new(),
        ),
    };
    let flet = Expr::Flet {
        definitions: vec![local.clone()],
        declarations: Vec::new(),
        body: vec![Expr::Call {
            operator: Operator::Name(local.name),
            arguments: Vec::new(),
        }],
    };
    let lowered = lower_toplevel(&Expr::Block {
        name: exit.clone(),
        body: vec![flet],
    })
    .expect("flet return analysis lowers");
    assert_verifies(&lowered.entry);
    assert!(
        lowered
            .entry
            .handler_regions
            .iter()
            .any(|region| region.kind == HandlerKind::Catch)
    );
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeClosure { .. }
    )));

    let label = LocalFunction {
        name: symbol("RECURSIVE"),
        lambda: lambda(
            vec![Expr::Let {
                sequential: false,
                bindings: vec![LetBinding {
                    name: symbol("N"),
                    value: Some(Expr::Constant(Literal::fixnum(1))),
                }],
                declarations: Vec::new(),
                body: vec![Expr::Variable(symbol("N"))],
            }],
            LambdaList::new(),
        ),
    };
    let labels = Expr::Labels {
        definitions: vec![label.clone()],
        declarations: Vec::new(),
        body: vec![Expr::Call {
            operator: Operator::Name(label.name),
            arguments: Vec::new(),
        }],
    };
    let labels_lowered = lower_toplevel(&labels).expect("labels analysis lowers");
    assert_verifies(&labels_lowered.entry);
    assert_eq!(labels_lowered.nested.len(), 1);
    assert!(any_op(&labels_lowered.entry, |kind| matches!(
        kind,
        OpKind::CallClosure { .. }
    )));
}

#[test]
fn capture_walks_function_namespaces_and_nested_value_forms() {
    let variable = symbol("VALUE");
    let function = symbol("HELPER");
    let nested = lambda(
        vec![Expr::Call {
            operator: Operator::Name(function.clone()),
            arguments: vec![Expr::Variable(variable.clone())],
        }],
        LambdaList::new(),
    );
    let local = LocalFunction {
        name: function,
        lambda: lambda(vec![Expr::Variable(variable.clone())], LambdaList::new()),
    };
    let expression = Expr::Let {
        sequential: false,
        bindings: vec![LetBinding {
            name: variable.clone(),
            value: Some(Expr::Constant(Literal::fixnum(8))),
        }],
        declarations: Vec::new(),
        body: vec![
            Expr::Flet {
                definitions: vec![local],
                declarations: Vec::new(),
                body: vec![Expr::Function(FunctionDesignator::Lambda(Box::new(nested)))],
            },
            Expr::Setq(vec![(variable.clone(), Expr::Constant(Literal::fixnum(9)))]),
            Expr::Variable(variable),
        ],
    };
    let lowered = lower_toplevel(&expression).expect("namespace capture lowers");
    assert_verifies(&lowered.entry);
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeClosure { .. }
    )));
    assert!(
        lowered
            .nested
            .iter()
            .any(|nested| { any_op(nested, |kind| matches!(kind, OpKind::LoadCapture { .. })) })
    );
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::StoreField { field: 0, .. }
    )));
}

#[test]
fn optional_supplied_aux_and_keyword_defaults_emit_runtime_prologue_ops() {
    let required = symbol("REQUIRED");
    let optional = symbol("OPTIONAL");
    let supplied = symbol("SUPPLIED-P");
    let keyword = SymbolRef::interned("KEYWORD", "VALUE");
    let key_name = symbol("VALUE");
    let aux = symbol("AUX");
    let list = LambdaList {
        required: vec![ParamName::Symbol(required)],
        optional: vec![OptionalParam {
            name: ParamName::Symbol(optional),
            default: Some(Expr::Constant(Literal::fixnum(2))),
            supplied_p: Some(ParamName::Symbol(supplied)),
        }],
        keys: vec![KeyParam {
            keyword,
            name: ParamName::Symbol(key_name.clone()),
            default: Some(Expr::Constant(Literal::fixnum(4))),
            supplied_p: None,
        }],
        aux: vec![AuxParam {
            name: ParamName::Symbol(aux),
            default: None,
        }],
        ..LambdaList::new()
    };
    let expression = Expr::Call {
        operator: Operator::Lambda(Box::new(lambda(vec![Expr::Variable(key_name)], list))),
        arguments: vec![Expr::Constant(Literal::fixnum(1))],
    };
    let lowered = lower_toplevel(&expression).expect("parameter prologue lowers");
    assert_verifies(&lowered.entry);
    let nested = &lowered.nested[0];
    assert_verifies(nested);
    assert!(any_op(
        nested,
        |kind| matches!(kind, OpKind::Builtin { name, .. } if name == "check-keywords")
    ));
    assert!(any_op(
        nested,
        |kind| matches!(kind, OpKind::Builtin { name, .. } if name == "keyword-supplied-p")
    ));
    assert!(any_op(nested, |kind| matches!(
        kind,
        OpKind::Compare { .. }
    )));
    assert!(any_terminator(nested, |term| matches!(
        term,
        Terminator::Branch { .. }
    )));
    assert!(
        nested
            .constants
            .iter()
            .any(|constant| matches!(constant, ncl_ir::Constant::Nil))
    );
}

#[test]
fn closure_designator_wrappers_route_funcall_to_closure_abi() {
    let inner = Expr::Lambda(Box::new(lambda(
        vec![Expr::Constant(Literal::fixnum(5))],
        LambdaList::new(),
    )));
    let wrapped = Expr::Locally {
        declarations: Vec::new(),
        body: vec![Expr::The {
            type_specifier: ncl_compiler_front::TypeSpecifier::new(Literal::T),
            value: Box::new(Expr::LoadTimeValue {
                form: Box::new(inner),
                read_only: true,
            }),
        }],
    };
    let expression = Expr::Call {
        operator: Operator::Name(SymbolRef::interned("COMMON-LISP", "FUNCALL")),
        arguments: vec![wrapped],
    };
    let lowered = lower_toplevel(&expression).expect("wrapped closure funcall lowers");
    assert_verifies(&lowered.entry);
    assert!(any_op(
        &lowered.entry,
        |kind| matches!(kind, OpKind::CallClosure { args, .. } if args.len() == 1)
    ));
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::MakeClosure { .. }
    )));
}

#[test]
fn multiple_value_prog1_and_global_function_cell_assignment_preserve_side_effects() {
    let expression = Expr::MultipleValueProg1 {
        first: Box::new(Expr::Constant(Literal::fixnum(1))),
        forms: vec![Expr::Constant(Literal::fixnum(2))],
    };
    let lowered = lower_toplevel(&expression).expect("multiple-value-prog1 lowers");
    assert_verifies(&lowered.entry);
    assert!(any_op(
        &lowered.entry,
        |kind| matches!(kind, OpKind::SetMultipleValues { values } if values.len() == 1)
    ));

    let assignment = Expr::Call {
        operator: Operator::Name(SymbolRef::interned("NCL-EXT", "FDEFINITION-SET")),
        arguments: vec![
            Expr::Constant(Literal::Symbol(symbol("FUNCTION"))),
            Expr::Constant(Literal::Symbol(symbol("TARGET"))),
        ],
    };
    let assigned = lower_toplevel(&assignment).expect("function cell assignment lowers");
    assert_verifies(&assigned.entry);
    assert!(any_op(
        &assigned.entry,
        |kind| matches!(kind, OpKind::StoreField { field, .. } if *field == u32::try_from(ncl_object::symbol_offset::FUNCTION).unwrap())
    ));
}

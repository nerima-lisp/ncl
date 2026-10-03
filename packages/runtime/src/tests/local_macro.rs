#![allow(clippy::expect_used, clippy::redundant_clone)]

use super::{
    BTreeMap, EntryCodes, LambdaList, LocalMacro, Operator, ParamName, SymbolRef, call_local_macro,
};
use ncl_compiler_front::ast::Expr;
use ncl_compiler_front::lambda_list::{AuxParam, KeyParam, OptionalParam};
use ncl_compiler_front::literal::{Literal, NumberLiteral};
use ncl_object::{
    DoubleFloat, Package, Runtime as ObjectRuntime, ThreadContext, Word, car, cdr, double_value,
    make_cons, simple_vector_length, simple_vector_ref, string_length, string_ref, symbol_name,
};

// check-added-lines: allow(expect_used) test-only setup
fn setup() -> (ObjectRuntime, ThreadContext) {
    let runtime = ObjectRuntime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    ncl_stdlib::register_all(&mut ctx, &runtime)
        .unwrap_or_else(|error| panic!("stdlib: {error:?}"));
    (runtime, ctx)
}

fn proper_list(ctx: &mut ThreadContext, runtime: &ObjectRuntime, items: &[Word]) -> Word {
    items.iter().rev().fold(Word::NIL, |tail, item| {
        make_cons(ctx, runtime, *item, tail).expect("list cell")
    })
}

fn call(
    ctx: &mut ThreadContext,
    runtime: &ObjectRuntime,
    definition: &LocalMacro,
    arguments: &[Word],
) -> Result<Word, ncl_compiler_front::FrontError> {
    let package = runtime
        .find_package(
            ctx,
            definition.name.package_name().expect("interned macro name"),
        )
        .expect("macro package");
    let (name, _) = Package::from_word(package)
        .intern(ctx, runtime, &definition.name.name)
        .expect("macro name");
    let argument_list = proper_list(ctx, runtime, arguments);
    let form = make_cons(ctx, runtime, name, argument_list).expect("macro form");
    let entry_codes: EntryCodes = BTreeMap::new();
    call_local_macro(ctx, runtime, &entry_codes, definition, form)
}

fn constant_definition(literal: Literal) -> LocalMacro {
    LocalMacro {
        name: SymbolRef::interned("COMMON-LISP-USER", "CONSTANT"),
        lambda_list: LambdaList::new(),
        declarations: Vec::new(),
        docstring: None,
        body: vec![Expr::Constant(literal)],
    }
}

/// `(macrolet ((double (x) (+ x 1))) (double 5))`'s expander, called
/// directly against `(double 5)`, under `gc_stress` and
/// `strict_forwarding`: every allocation this module makes forces a
/// collection first.
#[test]
fn local_macro_call_survives_gc_stress_and_strict_forwarding() {
    let (runtime, mut ctx) = setup();
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);

    let x = SymbolRef::interned("COMMON-LISP-USER", "X");
    let plus = SymbolRef::interned("COMMON-LISP", "+");
    let definition = LocalMacro {
        name: SymbolRef::interned("COMMON-LISP-USER", "DOUBLE"),
        lambda_list: LambdaList {
            required: vec![ParamName::Symbol(x.clone())],
            ..LambdaList::new()
        },
        declarations: Vec::new(),
        docstring: None,
        body: vec![Expr::Call {
            operator: Operator::Name(plus),
            arguments: vec![
                Expr::Variable(x),
                Expr::Constant(Literal::Number(NumberLiteral::Fixnum(1))),
            ],
        }],
    };

    let package = runtime
        .find_package(&ctx, "COMMON-LISP-USER")
        .unwrap_or_else(|| panic!("COMMON-LISP-USER package"));
    let (name_word, _) = Package::from_word(package)
        .intern(&mut ctx, &runtime, "DOUBLE")
        .unwrap_or_else(|error| panic!("intern: {error:?}"));
    let argument_word = Word::fixnum(5);
    let arguments = make_cons(&mut ctx, &runtime, argument_word, Word::NIL)
        .unwrap_or_else(|error| panic!("cons: {error:?}"));
    let form = make_cons(&mut ctx, &runtime, name_word, arguments)
        .unwrap_or_else(|error| panic!("cons: {error:?}"));

    let entry_codes: BTreeMap<usize, (Box<Word>, ncl_sys::RootToken)> = BTreeMap::new();
    let result = call_local_macro(&mut ctx, &runtime, &entry_codes, &definition, form)
        .unwrap_or_else(|error| panic!("call_local_macro: {error:?}"));
    assert_eq!(result, Word::fixnum(6));
}

#[test]
fn optional_parameters_use_defaults_and_supplied_p() {
    let (runtime, mut ctx) = setup();
    let x = SymbolRef::interned("COMMON-LISP-USER", "X");
    let y = SymbolRef::interned("COMMON-LISP-USER", "Y");
    let supplied = SymbolRef::interned("COMMON-LISP-USER", "SUPPLIED-P");
    let definition = LocalMacro {
        name: SymbolRef::interned("COMMON-LISP-USER", "CHOOSE"),
        lambda_list: LambdaList {
            required: vec![ParamName::Symbol(x.clone())],
            optional: vec![ncl_compiler_front::lambda_list::OptionalParam {
                name: ParamName::Symbol(y.clone()),
                default: Some(Expr::Constant(Literal::Number(NumberLiteral::Fixnum(9)))),
                supplied_p: Some(ParamName::Symbol(supplied.clone())),
            }],
            ..LambdaList::new()
        },
        declarations: Vec::new(),
        docstring: None,
        body: vec![Expr::If {
            test: Box::new(Expr::Variable(supplied)),
            then: Box::new(Expr::Variable(y)),
            otherwise: Some(Box::new(Expr::Constant(Literal::Number(
                NumberLiteral::Fixnum(0),
            )))),
        }],
    };
    let package = runtime
        .find_package(&ctx, "COMMON-LISP-USER")
        .expect("COMMON-LISP-USER package");
    let (name_word, _) = Package::from_word(package)
        .intern(&mut ctx, &runtime, "CHOOSE")
        .expect("macro name");
    let one = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).expect("one arg");
    let form = make_cons(&mut ctx, &runtime, name_word, one).expect("call form");
    let entry_codes = BTreeMap::new();
    assert_eq!(
        call_local_macro(&mut ctx, &runtime, &entry_codes, &definition, form)
            .expect("default expansion"),
        Word::fixnum(0)
    );

    let two = make_cons(&mut ctx, &runtime, Word::fixnum(2), Word::NIL).expect("second arg");
    let args = make_cons(&mut ctx, &runtime, Word::fixnum(1), two).expect("two args");
    let form = make_cons(&mut ctx, &runtime, name_word, args).expect("call form");
    assert_eq!(
        call_local_macro(&mut ctx, &runtime, &entry_codes, &definition, form)
            .expect("supplied expansion"),
        Word::fixnum(2)
    );
}

#[test]
fn argument_shape_errors_are_reported() {
    let (runtime, mut ctx) = setup();
    let x = SymbolRef::interned("COMMON-LISP-USER", "X");
    let definition = LocalMacro {
        name: SymbolRef::interned("COMMON-LISP-USER", "ONE"),
        lambda_list: LambdaList {
            required: vec![ParamName::Symbol(x)],
            ..LambdaList::new()
        },
        declarations: Vec::new(),
        docstring: None,
        body: vec![Expr::Constant(Literal::Number(NumberLiteral::Fixnum(1)))],
    };
    let package = runtime
        .find_package(&ctx, "COMMON-LISP-USER")
        .expect("COMMON-LISP-USER package");
    let (name_word, _) = Package::from_word(package)
        .intern(&mut ctx, &runtime, "ONE")
        .expect("macro name");
    let empty_call = make_cons(&mut ctx, &runtime, name_word, Word::NIL).expect("empty call");
    let entry_codes = BTreeMap::new();
    let too_few = call_local_macro(&mut ctx, &runtime, &entry_codes, &definition, empty_call)
        .expect_err("missing required argument");
    assert!(too_few.to_string().contains("too few arguments"));

    let extra = make_cons(&mut ctx, &runtime, Word::fixnum(2), Word::NIL).expect("extra arg");
    let args = make_cons(&mut ctx, &runtime, Word::fixnum(1), extra).expect("two args");
    let form = make_cons(&mut ctx, &runtime, name_word, args).expect("call form");
    let too_many = call_local_macro(&mut ctx, &runtime, &entry_codes, &definition, form)
        .expect_err("extra argument");
    assert!(too_many.to_string().contains("too many arguments"));
}

#[test]
#[allow(clippy::too_many_lines)]
fn materializes_supported_literals_with_their_object_shapes() {
    let (runtime, mut ctx) = setup();

    assert_eq!(
        call(&mut ctx, &runtime, &constant_definition(Literal::Nil), &[]).expect("nil"),
        Word::NIL
    );
    assert_eq!(
        call(&mut ctx, &runtime, &constant_definition(Literal::T), &[]).expect("t"),
        Word::TRUE
    );
    assert_eq!(
        call(
            &mut ctx,
            &runtime,
            &constant_definition(Literal::Character(u32::from('A'))),
            &[],
        )
        .expect("character"),
        Word::character(u32::from('A'))
    );
    assert_eq!(
        call(
            &mut ctx,
            &runtime,
            &constant_definition(Literal::fixnum(19)),
            &[],
        )
        .expect("fixnum"),
        Word::fixnum(19)
    );

    let double = call(
        &mut ctx,
        &runtime,
        &constant_definition(Literal::double(1.5)),
        &[],
    )
    .expect("double");
    assert_eq!(
        double_value(&ctx, DoubleFloat::from_word(double))
            .expect("double value")
            .to_bits(),
        1.5_f64.to_bits()
    );

    let string = call(
        &mut ctx,
        &runtime,
        &constant_definition(Literal::String("hello".chars().collect())),
        &[],
    )
    .expect("string");
    assert_eq!(string_length(&ctx, string).expect("string length"), 5);
    assert_eq!(string_ref(&ctx, string, 1).expect("string character"), 'e');

    let symbol = call(
        &mut ctx,
        &runtime,
        &constant_definition(Literal::Symbol(SymbolRef::keyword("ANSWER"))),
        &[],
    )
    .expect("symbol");
    let symbol_name = symbol_name(&ctx, symbol).expect("symbol name");
    assert_eq!(
        (0..string_length(&ctx, symbol_name).expect("symbol name length"))
            .map(|index| string_ref(&ctx, symbol_name, index).expect("symbol name character"))
            .collect::<String>(),
        "ANSWER"
    );

    let cons = call(
        &mut ctx,
        &runtime,
        &constant_definition(Literal::Cons(
            Box::new(Literal::fixnum(1)),
            Box::new(Literal::T),
        )),
        &[],
    )
    .expect("cons");
    assert_eq!(car(&ctx, cons).expect("cons car"), Word::fixnum(1));
    assert_eq!(cdr(&ctx, cons).expect("cons cdr"), Word::TRUE);

    let vector = call(
        &mut ctx,
        &runtime,
        &constant_definition(Literal::Vector(vec![Literal::fixnum(2), Literal::T])),
        &[],
    )
    .expect("vector");
    assert_eq!(
        simple_vector_length(&ctx, vector).expect("vector length"),
        2
    );
    assert_eq!(
        simple_vector_ref(&ctx, vector, 0).expect("vector first"),
        Word::fixnum(2)
    );
    assert_eq!(
        simple_vector_ref(&ctx, vector, 1).expect("vector second"),
        Word::TRUE
    );
}

#[test]
fn rejects_unsupported_literals_and_missing_packages() {
    let (runtime, mut ctx) = setup();
    for literal in [
        Literal::Array {
            dimensions: vec![1],
            element_type: ncl_object::ArrayElementType::T,
            elements: vec![Literal::Nil],
        },
        Literal::BitVector(vec![true]),
    ] {
        let error = call(&mut ctx, &runtime, &constant_definition(literal), &[])
            .expect_err("unsupported literal must fail");
        assert!(error.to_string().contains("literal shape is not supported"));
    }
    let error = call(
        &mut ctx,
        &runtime,
        &constant_definition(Literal::Symbol(SymbolRef::interned(
            "NO-SUCH-PACKAGE",
            "VALUE",
        ))),
        &[],
    )
    .expect_err("missing package must fail");
    assert_eq!(
        error.to_string(),
        "macro expansion of COMMON-LISP:MACROLET failed: Layout"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn evaluates_conditionals_progn_and_reports_unsupported_forms() {
    let (runtime, mut ctx) = setup();
    let definition = |body| LocalMacro {
        name: SymbolRef::interned("COMMON-LISP-USER", "CONTROL"),
        lambda_list: LambdaList::new(),
        declarations: Vec::new(),
        docstring: None,
        body: vec![body],
    };

    assert_eq!(
        call(
            &mut ctx,
            &runtime,
            &definition(Expr::If {
                test: Box::new(Expr::Constant(Literal::T)),
                then: Box::new(Expr::Constant(Literal::fixnum(1))),
                otherwise: Some(Box::new(Expr::Constant(Literal::fixnum(2)))),
            }),
            &[],
        )
        .expect("true branch"),
        Word::fixnum(1)
    );
    assert_eq!(
        call(
            &mut ctx,
            &runtime,
            &definition(Expr::If {
                test: Box::new(Expr::Constant(Literal::Nil)),
                then: Box::new(Expr::Constant(Literal::fixnum(1))),
                otherwise: Some(Box::new(Expr::Constant(Literal::fixnum(2)))),
            }),
            &[],
        )
        .expect("false branch"),
        Word::fixnum(2)
    );
    assert_eq!(
        call(
            &mut ctx,
            &runtime,
            &definition(Expr::If {
                test: Box::new(Expr::Constant(Literal::Nil)),
                then: Box::new(Expr::Constant(Literal::fixnum(1))),
                otherwise: None,
            }),
            &[],
        )
        .expect("implicit nil branch"),
        Word::NIL
    );
    assert_eq!(
        call(
            &mut ctx,
            &runtime,
            &definition(Expr::Progn(Vec::new())),
            &[],
        )
        .expect("empty progn"),
        Word::NIL
    );

    let unbound = definition(Expr::Variable(SymbolRef::interned(
        "COMMON-LISP-USER",
        "MISSING",
    )));
    assert!(
        call(&mut ctx, &runtime, &unbound, &[])
            .expect_err("unbound local macro variable")
            .to_string()
            .contains("not bound")
    );

    let unsupported_operator = definition(Expr::Call {
        operator: Operator::Lambda(Box::new(ncl_compiler_front::ast::LambdaExpr {
            lambda_list: LambdaList::new(),
            declarations: Vec::new(),
            docstring: None,
            body: Vec::new(),
        })),
        arguments: Vec::new(),
    });
    assert!(
        call(&mut ctx, &runtime, &unsupported_operator, &[])
            .expect_err("lambda operator is not interpreted")
            .to_string()
            .contains("not supported")
    );

    let uninterned_operator = definition(Expr::Call {
        operator: Operator::Name(SymbolRef::uninterned("FUNCTION", 1)),
        arguments: Vec::new(),
    });
    assert!(
        call(&mut ctx, &runtime, &uninterned_operator, &[])
            .expect_err("uninterned operator")
            .to_string()
            .contains("uninterned operator")
    );
}

#[test]
fn destructures_whole_environment_rest_body_and_optional_nil() {
    let (runtime, mut ctx) = setup();
    let whole = SymbolRef::interned("COMMON-LISP-USER", "WHOLE");
    let environment = SymbolRef::interned("COMMON-LISP-USER", "ENVIRONMENT");
    let rest = SymbolRef::interned("COMMON-LISP-USER", "REST");
    let definition = LocalMacro {
        name: SymbolRef::interned("COMMON-LISP-USER", "REST-MACRO"),
        lambda_list: LambdaList {
            rest: Some(ParamName::Symbol(rest.clone())),
            whole: Some(whole.clone()),
            environment: Some(environment.clone()),
            ..LambdaList::new()
        },
        declarations: Vec::new(),
        docstring: None,
        body: vec![Expr::Variable(rest)],
    };
    let result = call(
        &mut ctx,
        &runtime,
        &definition,
        &[Word::fixnum(1), Word::fixnum(2)],
    )
    .expect("rest binding");
    assert_eq!(car(&ctx, result).expect("rest first"), Word::fixnum(1));
    let tail = cdr(&ctx, result).expect("rest tail");
    assert_eq!(car(&ctx, tail).expect("rest second"), Word::fixnum(2));
    assert_eq!(cdr(&ctx, tail).expect("rest terminator"), Word::NIL);

    let environment_definition = LocalMacro {
        body: vec![Expr::Variable(environment)],
        ..definition.clone()
    };
    assert_eq!(
        call(&mut ctx, &runtime, &environment_definition, &[]).expect("environment binding"),
        Word::NIL
    );

    let whole_definition = LocalMacro {
        body: vec![Expr::Variable(whole)],
        ..definition.clone()
    };
    let whole_result =
        call(&mut ctx, &runtime, &whole_definition, &[Word::fixnum(3)]).expect("whole binding");
    let whole_arguments = cdr(&ctx, whole_result).expect("whole arguments");
    assert_eq!(
        car(&ctx, whole_arguments).expect("whole argument"),
        Word::fixnum(3)
    );
    assert_eq!(
        cdr(&ctx, whole_arguments).expect("whole terminator"),
        Word::NIL
    );

    let body_name = SymbolRef::interned("COMMON-LISP-USER", "BODY");
    let body_definition = LocalMacro {
        lambda_list: LambdaList {
            body: Some(ParamName::Symbol(body_name.clone())),
            ..LambdaList::new()
        },
        body: vec![Expr::Variable(body_name)],
        ..definition.clone()
    };
    let body_result =
        call(&mut ctx, &runtime, &body_definition, &[Word::fixnum(4)]).expect("body binding");
    assert_eq!(car(&ctx, body_result).expect("body first"), Word::fixnum(4));
    assert_eq!(cdr(&ctx, body_result).expect("body terminator"), Word::NIL);

    let optional = LocalMacro {
        lambda_list: LambdaList {
            optional: vec![OptionalParam {
                name: ParamName::Symbol(SymbolRef::interned("COMMON-LISP-USER", "OPTIONAL")),
                default: None,
                supplied_p: None,
            }],
            ..LambdaList::new()
        },
        body: vec![Expr::Variable(SymbolRef::interned(
            "COMMON-LISP-USER",
            "OPTIONAL",
        ))],
        ..definition
    };
    assert_eq!(
        call(&mut ctx, &runtime, &optional, &[]).expect("optional nil"),
        Word::NIL
    );
}

#[test]
fn rejects_key_aux_and_nested_destructuring() {
    let (runtime, mut ctx) = setup();
    let key_definition = LocalMacro {
        name: SymbolRef::interned("COMMON-LISP-USER", "KEY"),
        lambda_list: LambdaList {
            keys: vec![KeyParam {
                keyword: SymbolRef::keyword("VALUE"),
                name: ParamName::Symbol(SymbolRef::interned("COMMON-LISP-USER", "VALUE")),
                default: None,
                supplied_p: None,
            }],
            ..LambdaList::new()
        },
        declarations: Vec::new(),
        docstring: None,
        body: vec![Expr::Constant(Literal::Nil)],
    };
    let aux_definition = LocalMacro {
        lambda_list: LambdaList {
            aux: vec![AuxParam {
                name: ParamName::Symbol(SymbolRef::interned("COMMON-LISP-USER", "VALUE")),
                default: None,
            }],
            ..LambdaList::new()
        },
        ..key_definition.clone()
    };
    let nested = LocalMacro {
        lambda_list: LambdaList {
            required: vec![ParamName::Pattern(Box::new(LambdaList::new()))],
            ..LambdaList::new()
        },
        ..key_definition.clone()
    };
    for definition in [key_definition, aux_definition] {
        assert!(
            call(&mut ctx, &runtime, &definition, &[])
                .expect_err("unsupported lambda-list feature")
                .to_string()
                .contains("not supported")
        );
    }

    assert!(
        call(&mut ctx, &runtime, &nested, &[Word::NIL])
            .expect_err("nested pattern")
            .to_string()
            .contains("nested destructuring")
    );
}

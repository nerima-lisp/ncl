#![allow(clippy::expect_used, clippy::redundant_clone)]

use super::{BTreeMap, LambdaList, LocalMacro, Operator, ParamName, SymbolRef, call_local_macro};
use ncl_compiler_front::ast::Expr;
use ncl_compiler_front::literal::{Literal, NumberLiteral};
use ncl_object::{Package, Runtime as ObjectRuntime, ThreadContext, Word, make_cons};

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

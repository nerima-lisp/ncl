use super::{
    BTreeMap, LambdaList, LocalMacro, Operator, ParamName, SymbolRef, call_local_macro, materialize,
};
use ncl_compiler_front::FrontError;
use ncl_compiler_front::ast::Expr;
use ncl_compiler_front::literal::{Literal, NumberLiteral};
use ncl_object::{
    ArrayElementType, Package, Runtime as ObjectRuntime, ThreadContext, Word, make_cons,
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
    let result = call_local_macro(
        &mut ctx,
        &runtime,
        &entry_codes,
        &definition,
        form,
        Word::NIL,
    )
    .unwrap_or_else(|error| panic!("call_local_macro: {error:?}"));
    assert_eq!(result, Word::fixnum(6));
}

#[test]
fn materialize_rejects_array_literal_shapes_in_local_macro_bodies() -> Result<(), String> {
    let (runtime, mut ctx) = setup();
    let literals = [
        Literal::Array {
            identity: 0,
            dimensions: vec![2],
            element_type: ArrayElementType::T,
            elements: vec![Literal::T, Literal::Nil],
        },
        Literal::SpecializedArray {
            identity: 1,
            element_type: ArrayElementType::Bit,
            elements: vec![Literal::fixnum(0), Literal::fixnum(1)],
        },
    ];

    for literal in &literals {
        let error = match materialize(&mut ctx, &runtime, &mut Vec::new(), literal) {
            Err(error) => error,
            Ok(index) => {
                return Err(format!(
                    "array literal unexpectedly materialized at held index {index}"
                ));
            }
        };
        let FrontError::MacroExpansion { name, detail } = error else {
            return Err(format!("expected MacroExpansion, got {error:?}"));
        };
        if name != SymbolRef::interned("COMMON-LISP", "MACROLET") {
            return Err(format!("unexpected error name: {name:?}"));
        }
        if detail != "macrolet: this literal shape is not supported in a local macro body" {
            return Err(format!("unexpected error detail: {detail}"));
        }
    }

    Ok(())
}

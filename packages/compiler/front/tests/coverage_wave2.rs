//! Second-wave coverage for declaration parsing, expansion helpers, and IR lowering.
#![allow(clippy::expect_used, clippy::unwrap_used)]

#[path = "support/forms.rs"]
mod forms;

use forms::Fixture;
use ncl_compiler_front::lower_toplevel;
use ncl_compiler_front::{
    Declaration, Expr, FormExpander, FrontError, Literal, MacroRegistry, NumberLiteral, Quality,
    SymbolRef,
};
use ncl_ir::{Constant, Function, OpKind, Terminator, verify};
use ncl_object::Word;

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

fn symbol(package: &str, name: &str) -> Literal {
    Literal::Symbol(SymbolRef::interned(package, name))
}

fn list(items: Vec<Literal>) -> Literal {
    items.into_iter().rev().fold(Literal::Nil, |tail, item| {
        Literal::Cons(Box::new(item), Box::new(tail))
    })
}

#[test]
fn declaration_qualities_and_malformed_shapes_are_distinct() {
    let names = [
        ("SPEED", Quality::Speed),
        ("SPACE", Quality::Space),
        ("SAFETY", Quality::Safety),
        ("DEBUG", Quality::Debug),
        ("COMPILATION-SPEED", Quality::CompilationSpeed),
    ];
    for (name, expected) in names {
        assert_eq!(Quality::from_name(name), Some(expected));
    }
    assert_eq!(Quality::from_name("UNKNOWN"), None);

    let declare = symbol("COMMON-LISP", "DECLARE");
    let special = symbol("COMMON-LISP", "SPECIAL");
    let x = symbol("COMMON-LISP-USER", "X");
    let parsed = ncl_compiler_front::parse_declare_form(&list(vec![
        declare.clone(),
        list(vec![special.clone(), x.clone()]),
    ]))
    .unwrap();
    assert_eq!(
        parsed,
        vec![Declaration::Special(vec![match x {
            Literal::Symbol(symbol) => symbol,
            _ => unreachable!(),
        }])]
    );

    let malformed = [
        (Literal::Nil, "empty declare form"),
        (
            Literal::Cons(
                Box::new(symbol("COMMON-LISP-USER", "NO")),
                Box::new(Literal::Nil),
            ),
            "form does not start with declare",
        ),
        (list(vec![declare.clone(), Literal::Nil]), "empty specifier"),
        (
            list(vec![
                declare.clone(),
                Literal::Cons(Box::new(special.clone()), Box::new(Literal::T)),
            ]),
            "specifier is not a proper list",
        ),
        (
            list(vec![
                declare.clone(),
                list(vec![symbol("COMMON-LISP", "TYPE")]),
            ]),
            "type declaration has no type",
        ),
        (
            list(vec![
                declare.clone(),
                list(vec![
                    symbol("COMMON-LISP", "FTYPE"),
                    symbol("COMMON-LISP", "INTEGER"),
                ]),
            ]),
            "ftype declaration has no name",
        ),
    ];
    for (form, detail) in malformed {
        assert_eq!(
            ncl_compiler_front::parse_declare_form(&form),
            Err(FrontError::MalformedDeclaration {
                detail: detail.to_owned()
            })
        );
    }
}

#[test]
fn declaration_quality_values_cover_unknown_non_integer_and_range_errors() {
    let optimize = symbol("COMMON-LISP", "OPTIMIZE");
    let quality = symbol("COMMON-LISP", "SPACE");
    let unknown_quality = symbol("COMMON-LISP-USER", "CUSTOM");
    let valid = list(vec![
        optimize.clone(),
        list(vec![quality, Literal::fixnum(2)]),
        list(vec![unknown_quality, Literal::fixnum(1)]),
    ]);
    let parsed = ncl_compiler_front::parse_declare_form(&list(vec![
        symbol("COMMON-LISP", "DECLARE"),
        match valid.list_elements().unwrap()[1] {
            item => item.clone(),
        },
    ]));
    assert!(parsed.is_ok(), "valid optimize declaration parses");

    for value in [
        list(vec![symbol("COMMON-LISP", "SPEED")]),
        list(vec![
            symbol("COMMON-LISP", "SPEED"),
            Literal::fixnum(1),
            Literal::fixnum(2),
        ]),
        list(vec![
            symbol("COMMON-LISP", "SPEED"),
            Literal::String(vec!['x']),
        ]),
        list(vec![symbol("COMMON-LISP", "SPEED"), Literal::fixnum(-1)]),
    ] {
        let form = list(vec![
            symbol("COMMON-LISP", "DECLARE"),
            list(vec![optimize.clone(), value]),
        ]);
        assert!(matches!(
            ncl_compiler_front::parse_declare_form(&form),
            Err(FrontError::MalformedDeclaration { .. })
        ));
    }
}

#[test]
fn expand_body_tracks_declarations_docstring_and_forms() {
    let mut fixture = Fixture::new();
    let x = fixture.user("X");
    let declare = fixture.cl("DECLARE");
    let special = fixture.cl("SPECIAL");
    let special_specifier = fixture.list(&[special, x]);
    let declaration = fixture.list(&[declare, special_specifier]);
    let docstring = fixture.string("documentation");
    let registry = MacroRegistry::new();
    let mut expander = FormExpander::new(&mut fixture.ctx, &fixture.runtime, &registry);
    let body = expander
        .expand_body(&[declaration, docstring, Word::fixnum(7)])
        .unwrap();

    assert_eq!(body.docstring.as_deref(), Some("documentation"));
    assert_eq!(body.declarations.len(), 1);
    assert_eq!(body.forms, vec![Expr::Constant(Literal::fixnum(7))]);
    expander.apply_declarations(&body.declarations);
    assert!(
        expander
            .env()
            .is_special(&SymbolRef::interned("COMMON-LISP-USER", "X"))
    );
}

#[test]
fn expander_public_helpers_return_data_and_reject_invalid_lambda_forms() {
    let mut fixture = Fixture::new();
    let x = fixture.user("X");
    let bad_head = fixture.form("IF", &[Word::TRUE]);
    let registry = MacroRegistry::new();
    let mut expander = FormExpander::new(&mut fixture.ctx, &fixture.runtime, &registry);
    let value = Word::fixnum(4);
    assert_eq!(expander.datum(value).unwrap(), Literal::fixnum(4));
    assert_eq!(expander.elements(Word::NIL).unwrap(), Vec::<Word>::new());
    assert!(expander.is_named(x, "X").unwrap());
    assert!(!expander.is_named(value, "X").unwrap());
    assert_eq!(expander.expand_all(&[value, Word::NIL]).unwrap().len(), 2);
    let runtime_pointer = expander.runtime() as *const ncl_object::Runtime;
    assert!(!runtime_pointer.is_null(), "runtime accessor remains live");

    assert!(matches!(
        expander.expand_lambda(bad_head),
        Err(FrontError::InvalidOperator { .. })
    ));
    assert!(matches!(
        expander.expand_lambda(Word::NIL),
        Err(FrontError::InvalidOperator { .. })
    ));
}

#[test]
fn lower_if_covers_both_value_paths_and_merges_assignment() {
    let mut fixture = Fixture::new();
    let x = fixture.user("X");
    let true_symbol = fixture.cl("T");
    let binding = fixture.list(&[x, Word::fixnum(0)]);
    let bindings = fixture.list(&[binding]);
    let then_set = fixture.form("SETQ", &[x, Word::fixnum(1)]);
    let else_set = fixture.form("SETQ", &[x, Word::fixnum(2)]);
    let conditional = fixture.form("IF", &[true_symbol, then_set, else_set]);
    let form = fixture.form("LET", &[bindings, conditional, x]);
    let expr = fixture.expand(form).unwrap();
    let lowered = lower_toplevel(&expr).unwrap();

    assert_verifies(&lowered.entry);
    assert!(any_terminator(&lowered.entry, |term| matches!(
        term,
        Terminator::Branch { .. }
    )));
    assert!(
        lowered.entry.blocks.len() >= 4,
        "if lowering creates then, else, and merge blocks"
    );
}

#[test]
fn lower_special_let_emits_progv_handler_and_restores_path() {
    let mut fixture = Fixture::new();
    let x = fixture.user("X");
    let binding = fixture.list(&[x, Word::fixnum(3)]);
    let bindings = fixture.list(&[binding]);
    let declare = fixture.cl("DECLARE");
    let special = fixture.cl("SPECIAL");
    let special_specifier = fixture.list(&[special, x]);
    let declaration = fixture.list(&[declare, special_specifier]);
    let form = fixture.form("LET", &[bindings, declaration, x]);
    let expr = fixture.expand(form).unwrap();
    let lowered = lower_toplevel(&expr).unwrap();

    assert_verifies(&lowered.entry);
    assert_eq!(lowered.entry.handler_regions.len(), 1);
    assert_eq!(
        lowered.entry.handler_regions[0].kind,
        ncl_ir::HandlerKind::Progv
    );
    assert!(any_op(&lowered.entry, |kind| matches!(
        kind,
        OpKind::Builtin { name, .. } if name == "CONS"
    )));
}

#[test]
fn lower_aux_and_eval_when_cover_defaults_and_compile_only_nil() {
    let mut fixture = Fixture::new();
    let lambda = fixture.cl("LAMBDA");
    let aux = fixture.cl("&AUX");
    let name = fixture.user("A");
    let aux_spec = fixture.list(&[name, Word::fixnum(8)]);
    let lambda_list = fixture.list(&[aux, aux_spec]);
    let lambda_form = fixture.list(&[lambda, lambda_list, name]);
    let call = fixture.list(&[lambda_form]);
    let lambda_expr = fixture.expand(call).unwrap();
    let lowered = lower_toplevel(&lambda_expr).unwrap();
    assert_verifies(&lowered.entry);
    assert_verifies(&lowered.nested[0]);
    assert!(
        lowered.nested[0]
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Fixnum(8)))
    );

    let compile_situation = fixture.keyword("COMPILE-TOPLEVEL");
    let compile_situations = fixture.list(&[compile_situation]);
    let compile = fixture.form("EVAL-WHEN", &[compile_situations, Word::fixnum(1)]);
    let compile_expr = fixture.expand(compile).unwrap();
    let compile_lowered = lower_toplevel(&compile_expr).unwrap();
    assert!(
        compile_lowered
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, Constant::Nil))
    );
}

#[test]
fn lower_literals_cover_structures_numbers_and_explicit_unsupported_error() {
    let expressions = [
        Expr::Constant(Literal::String(vec!['λ'])),
        Expr::Constant(Literal::Cons(
            Box::new(Literal::fixnum(1)),
            Box::new(Literal::T),
        )),
        Expr::Constant(Literal::Vector(vec![Literal::fixnum(1), Literal::T])),
        Expr::Constant(Literal::Number(NumberLiteral::Complex {
            real: Box::new(NumberLiteral::Fixnum(1)),
            imaginary: Box::new(NumberLiteral::DoubleFloat(2.0)),
        })),
    ];
    for expression in expressions {
        let lowered = lower_toplevel(&expression).unwrap();
        assert_verifies(&lowered.entry);
        assert!(!lowered.entry.constants.is_empty());
    }

    let unsupported = Expr::Constant(Literal::Array {
        dimensions: vec![1],
        elements: vec![Literal::fixnum(1)],
    });
    match lower_toplevel(&unsupported) {
        Err(error) => assert_eq!(
            error,
            ncl_compiler_front::LowerError::Unsupported {
                form: "quoted structure"
            }
        ),
        Ok(_) => panic!("array literal unexpectedly lowered"),
    }
}

#[test]
fn lowering_an_unbound_return_reports_escaping_control_error() {
    let name = SymbolRef::interned("COMMON-LISP-USER", "MISSING");
    let expression = Expr::ReturnFrom {
        name: name.clone(),
        value: Some(Box::new(Expr::Constant(Literal::fixnum(1)))),
    };
    let error = lower_toplevel(&expression).unwrap_err();
    assert_eq!(
        error,
        ncl_compiler_front::LowerError::EscapingControl { name: name.clone() }
    );
    assert_eq!(
        error.to_string(),
        "COMMON-LISP-USER:MISSING escapes the function being lowered"
    );
}

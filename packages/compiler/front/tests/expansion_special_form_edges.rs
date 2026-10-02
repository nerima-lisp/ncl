//! Boundary cases for expansion helpers, declarations, and special forms.
#![allow(clippy::expect_used, clippy::unwrap_used)]

#[path = "support/forms.rs"]
mod forms;

use forms::Fixture;
use ncl_compiler_front::{
    Declaration, Expr, FormExpander, FrontError, Literal, MacroRegistry, Quality, SymbolRef,
};
use ncl_object::{Word, make_cons};

fn declare(f: &mut Fixture, specifiers: &[Word]) -> Word {
    let mut elements = vec![f.cl("DECLARE")];
    elements.extend_from_slice(specifiers);
    f.list(&elements)
}

fn declaration_error(f: &mut Fixture, specifier: Word) -> FrontError {
    let form = declare(f, &[specifier]);
    f.declarations(form).unwrap_err()
}

fn assert_malformed(error: FrontError) {
    assert!(matches!(error, FrontError::MalformedForm { .. }));
}

#[test]
fn expansion_helpers_handle_empty_bodies_and_non_docstrings() {
    let mut f = Fixture::new();
    let registry = MacroRegistry::new();
    let mut expander = FormExpander::new(&mut f.ctx, &f.runtime, &registry);

    let body = expander.expand_body(&[Word::fixnum(7)]).unwrap();
    assert_eq!(body.docstring, None);
    assert_eq!(body.declarations, Vec::<Declaration>::new());
    assert_eq!(body.forms, vec![Expr::Constant(Literal::fixnum(7))]);

    let declared = expander.expand_declared_body(&[]).unwrap();
    assert_eq!(declared.docstring, None);
    assert!(declared.forms.is_empty());
}

#[test]
fn declaration_edges_reject_non_symbols_and_preserve_all_quality_names() {
    let mut f = Fixture::new();
    for keyword in [
        "SPECIAL",
        "INLINE",
        "NOTINLINE",
        "DYNAMIC-EXTENT",
        "IGNORE",
        "IGNORABLE",
    ] {
        let head = f.cl(keyword);
        let specifier = f.list(&[head, Word::fixnum(1)]);
        assert!(matches!(
            declaration_error(&mut f, specifier),
            FrontError::MalformedDeclaration { .. }
        ));
    }

    let type_head = f.cl("TYPE");
    let integer = f.cl("INTEGER");
    let type_specifier = f.list(&[type_head, integer, Word::fixnum(1)]);
    assert!(matches!(
        declaration_error(&mut f, type_specifier),
        FrontError::MalformedDeclaration { .. }
    ));

    let ftype_head = f.cl("FTYPE");
    let integer = f.cl("INTEGER");
    let ftype_specifier = f.list(&[ftype_head, integer, Word::fixnum(1)]);
    assert!(matches!(
        declaration_error(&mut f, ftype_specifier),
        FrontError::MalformedDeclaration { .. }
    ));

    for quality in ["SPEED", "SPACE", "SAFETY", "DEBUG", "COMPILATION-SPEED"] {
        let quality_word = f.cl(quality);
        let item = f.list(&[quality_word, Word::fixnum(0)]);
        let optimize_head = f.cl("OPTIMIZE");
        let optimize = f.list(&[optimize_head, item]);
        let declaration = declare(&mut f, &[optimize]);
        let parsed = f.declarations(declaration).unwrap();
        assert!(matches!(
            parsed.as_slice(),
            [Declaration::Optimize(qualities)] if qualities[0].value == 0
        ));
    }

    let custom = f.user("CUSTOM-QUALITY");
    let item = f.list(&[custom, Word::fixnum(2)]);
    let optimize_head = f.cl("OPTIMIZE");
    let optimize = f.list(&[optimize_head, item]);
    let declaration = declare(&mut f, &[optimize]);
    assert_eq!(
        f.declarations(declaration).unwrap(),
        vec![Declaration::Optimize(vec![
            ncl_compiler_front::OptimizeQuality {
                quality: Quality::Unknown,
                value: 2,
            }
        ])]
    );
}

#[test]
fn declaration_quality_boundaries_report_malformed_declarations() {
    let mut f = Fixture::new();
    let non_declare = f.form("IF", &[]);
    assert!(matches!(
        f.declarations(non_declare),
        Err(FrontError::MalformedDeclaration { .. })
    ));

    let optimize = f.cl("OPTIMIZE");
    let speed = f.cl("SPEED");
    let not_integer = f.string("not-an-integer");
    let items = [
        f.list(&[speed]),
        f.list(&[speed, Word::fixnum(1), Word::fixnum(2)]),
        f.list(&[speed, not_integer]),
        f.list(&[speed, Word::fixnum(-1)]),
    ];
    for item in items {
        let specifier = f.list(&[optimize, item]);
        assert!(matches!(
            declaration_error(&mut f, specifier),
            FrontError::MalformedDeclaration { .. }
        ));
    }
}

#[test]
fn control_edges_cover_eval_when_variants_and_load_time_flags() {
    let mut f = Fixture::new();
    let compile = f.keyword("COMPILE-TOPLEVEL");
    let load = f.keyword("LOAD-TOPLEVEL");
    let execute = f.keyword("EXECUTE");
    let eval = f.keyword("EVAL");
    let situations = f.list(&[compile, load, execute, eval]);
    let eval_when = f.form("EVAL-WHEN", &[situations]);
    match f.expand(eval_when).unwrap() {
        Expr::EvalWhen { situations, body } => {
            assert_eq!(situations.len(), 4);
            assert!(body.is_empty());
        }
        other => panic!("expected EvalWhen, got {other:?}"),
    }

    let form = f.form("LOAD-TIME-VALUE", &[Word::fixnum(1), Word::NIL]);
    assert!(matches!(
        f.expand(form).unwrap(),
        Expr::LoadTimeValue {
            read_only: false,
            ..
        }
    ));

    let protected = f.user("PROTECTED");
    let cleanup = f.user("CLEANUP");
    let operator = f.intern("NCL-SYS", "NLX-PROTECT");
    let form = f.list(&[operator, protected, cleanup]);
    assert!(matches!(
        f.expand(form).unwrap(),
        Expr::UnwindProtect { cleanup, .. } if cleanup.len() == 1
    ));
}

#[test]
fn binding_edges_reject_empty_and_malformed_definitions() {
    let mut f = Fixture::new();
    let invalid_binding = f.list(&[Word::fixnum(1)]);
    let bindings = f.list(&[invalid_binding]);
    let let_form = f.form("LET", &[bindings]);
    assert_eq!(
        f.expand(let_form),
        Err(FrontError::Object(ncl_object::ObjectError::TypeError))
    );

    let name = f.user("X");
    let too_many = f.list(&[name, Word::fixnum(1), Word::fixnum(2)]);
    let bindings = f.list(&[too_many]);
    let let_star = f.form("LET*", &[bindings]);
    assert_malformed(f.expand(let_star).unwrap_err());

    let invalid_definition = f.list(&[Word::fixnum(1), Word::NIL]);
    let definitions = f.list(&[invalid_definition]);
    let flet = f.form("FLET", &[definitions]);
    assert!(matches!(
        f.expand(flet).unwrap_err(),
        FrontError::Object(ncl_object::ObjectError::TypeError)
    ));

    let empty_macro_definition = f.list(&[]);
    let definitions = f.list(&[empty_macro_definition]);
    let macrolet = f.form("MACROLET", &[definitions]);
    assert_malformed(f.expand(macrolet).unwrap_err());

    let bad_symbol_macro = f.list(&[name]);
    let definitions = f.list(&[bad_symbol_macro]);
    let symbol_macrolet = f.form("SYMBOL-MACROLET", &[definitions]);
    assert_malformed(f.expand(symbol_macrolet).unwrap_err());
}

#[test]
fn function_and_control_edges_reject_invalid_shapes() {
    let mut f = Fixture::new();
    let function_name = f.user("F");
    let argument_name = f.user("X");
    let compound_name = f.list(&[function_name, argument_name]);
    let function = f.form("FUNCTION", &[compound_name]);
    assert_malformed(f.expand(function).unwrap_err());

    let setq_name = f.user("X");
    let odd_setq = f.form("SETQ", &[setq_name]);
    assert!(matches!(
        f.expand(odd_setq),
        Err(FrontError::WrongNumberOfForms { .. })
    ));

    let symbol_macro_name = f.user("X");
    let symbol_macro = f.list(&[symbol_macro_name, Word::fixnum(1)]);
    let definitions = f.list(&[symbol_macro]);
    let setq = f.form("SETQ", &[symbol_macro_name, Word::fixnum(2)]);
    let form = f.form("SYMBOL-MACROLET", &[definitions, setq]);
    assert_malformed(f.expand(form).unwrap_err());

    let progn = f.cl("PROGN");
    let dotted = make_cons(&mut f.ctx, &f.runtime, progn, Word::fixnum(1)).unwrap();
    assert_eq!(f.expand(dotted), Err(FrontError::ImproperList));
}

#[test]
fn expansion_lambda_boundaries_and_type_application_have_exact_results() {
    let mut f = Fixture::new();
    let wrong_head = f.form("IF", &[Word::TRUE]);
    let missing_list = f.form("LAMBDA", &[]);
    let registry = MacroRegistry::new();
    let mut expander = FormExpander::new(&mut f.ctx, &f.runtime, &registry);

    assert_eq!(
        expander.expand_lambda(Word::NIL).unwrap_err(),
        FrontError::InvalidOperator {
            detail: "empty lambda expression".to_owned(),
        }
    );
    assert_eq!(
        expander.expand_lambda(wrong_head).unwrap_err(),
        FrontError::InvalidOperator {
            detail: "lambda expression does not start with lambda".to_owned(),
        }
    );
    assert_eq!(
        expander.expand_lambda(missing_list).unwrap_err(),
        FrontError::WrongNumberOfForms {
            operator: SymbolRef::interned("COMMON-LISP", "LAMBDA"),
            expected: "a lambda list",
            found: 0,
        }
    );

    let x = SymbolRef::interned("COMMON-LISP-USER", "X");
    let type_specifier = ncl_compiler_front::TypeSpecifier::new(Literal::Symbol(
        SymbolRef::interned("COMMON-LISP", "INTEGER"),
    ));
    expander.apply_declarations(&[Declaration::Type {
        type_specifier: type_specifier.clone(),
        names: vec![x.clone()],
    }]);
    assert_eq!(expander.env().variable_type(&x), Some(&type_specifier));
    assert!(!expander.is_named(Word::fixnum(1), "X").unwrap());
}

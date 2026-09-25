//! Declaration specifiers parse, and declarations reach the environment.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "integration tests assert on results"
)]

#[path = "support/forms.rs"]
mod forms;

use forms::Fixture;

use ncl_compiler_front::{Declaration, Expr, LexicalEnv, Quality, TypeSpecifier};
use ncl_object::Word;

/// Build a `(declare specifier*)` form.
fn declare(f: &mut Fixture, specifiers: &[Word]) -> Word {
    let head = f.cl("DECLARE");
    let mut elements = vec![head];
    elements.extend_from_slice(specifiers);
    f.list(&elements)
}

/// Parse one declaration specifier out of a `(declare ...)` form.
fn parse_one(f: &mut Fixture, specifier: Word) -> Declaration {
    let form = declare(f, &[specifier]);
    let declarations = f.declarations(form).unwrap();
    assert_eq!(declarations.len(), 1);
    declarations.into_iter().next().unwrap()
}

#[test]
fn special_parses_its_variables() {
    let mut f = Fixture::new();
    let special = f.cl("SPECIAL");
    let x = f.user("X");
    let specifier = f.list(&[special, x]);
    match parse_one(&mut f, specifier) {
        Declaration::Special(names) => {
            assert_eq!(names.len(), 1);
            assert_eq!(names[0].name, "X");
        }
        other => panic!("expected Special, got {other:?}"),
    }
}

#[test]
fn type_parses_its_specifier_and_variables() {
    let mut f = Fixture::new();
    let type_name = f.cl("TYPE");
    let integer = f.cl("INTEGER");
    let x = f.user("X");
    let specifier = f.list(&[type_name, integer, x]);
    match parse_one(&mut f, specifier) {
        Declaration::Type {
            type_specifier,
            names,
        } => {
            assert_eq!(names.len(), 1);
            assert_eq!(
                type_specifier,
                TypeSpecifier::new(ncl_compiler_front::Literal::Symbol(
                    ncl_compiler_front::SymbolRef::interned("COMMON-LISP", "INTEGER")
                ))
            );
        }
        other => panic!("expected Type, got {other:?}"),
    }
}

#[test]
fn ftype_parses_its_specifier_and_name() {
    let mut f = Fixture::new();
    let ftype = f.cl("FTYPE");
    let integer = f.cl("INTEGER");
    let name = f.user("F");
    let specifier = f.list(&[ftype, integer, name]);
    assert!(matches!(
        parse_one(&mut f, specifier),
        Declaration::Ftype { .. }
    ));
}

#[test]
fn inline_notinline_dynamic_extent_ignore_and_ignorable_parse() {
    let mut f = Fixture::new();
    let name = f.user("X");
    for keyword in [
        "INLINE",
        "NOTINLINE",
        "DYNAMIC-EXTENT",
        "IGNORE",
        "IGNORABLE",
    ] {
        let head = f.cl(keyword);
        let specifier = f.list(&[head, name]);
        let declaration = parse_one(&mut f, specifier);
        assert!(
            matches!(
                declaration,
                Declaration::Inline(_)
                    | Declaration::Notinline(_)
                    | Declaration::DynamicExtent(_)
                    | Declaration::Ignore(_)
                    | Declaration::Ignorable(_)
            ),
            "{keyword} produced {declaration:?}"
        );
    }
}

#[test]
fn optimize_parses_qualities() {
    let mut f = Fixture::new();
    let optimize = f.cl("OPTIMIZE");
    let speed = f.cl("SPEED");
    let value = Word::fixnum(3);
    let quality = f.list(&[speed, value]);
    let specifier = f.list(&[optimize, quality]);
    match parse_one(&mut f, specifier) {
        Declaration::Optimize(qualities) => {
            assert_eq!(qualities.len(), 1);
            assert_eq!(qualities[0].quality, Quality::Speed);
            assert_eq!(qualities[0].value, 3);
        }
        other => panic!("expected Optimize, got {other:?}"),
    }
}

#[test]
fn an_unknown_specifier_is_retained() {
    let mut f = Fixture::new();
    let unknown = f.intern("NCL-EXT", "SOME-DECLARATION");
    let argument = f.user("X");
    let specifier = f.list(&[unknown, argument]);
    match parse_one(&mut f, specifier) {
        Declaration::Unknown { name, arguments } => {
            assert_eq!(name.name, "SOME-DECLARATION");
            assert_eq!(arguments.len(), 1);
        }
        other => panic!("expected Unknown, got {other:?}"),
    }
}

#[test]
fn a_let_body_keeps_its_declarations() {
    let mut f = Fixture::new();
    let x = f.user("X");
    let binding = f.list(&[x, Word::fixnum(1)]);
    let bindings = f.list(&[binding]);
    let special = f.cl("SPECIAL");
    let specifier = f.list(&[special, x]);
    let declaration = declare(&mut f, &[specifier]);
    let body = f.user("X");
    let form = f.form("LET", &[bindings, declaration, body]);
    match f.expand(form).unwrap() {
        Expr::Let {
            declarations, body, ..
        } => {
            assert_eq!(declarations.len(), 1);
            assert!(declarations[0].is_special());
            assert_eq!(body.len(), 1);
        }
        other => panic!("expected Let, got {other:?}"),
    }
}

#[test]
fn the_environment_reports_special_and_typed_variables() {
    let mut env = LexicalEnv::new();
    let x = ncl_compiler_front::SymbolRef::interned("COMMON-LISP-USER", "X");
    env.push_scope();
    env.bind_special(x.clone());
    assert!(env.is_special(&x));
    assert!(env.lookup_variable(&x).unwrap().is_special());

    let y = ncl_compiler_front::SymbolRef::interned("COMMON-LISP-USER", "Y");
    let specifier = TypeSpecifier::new(ncl_compiler_front::Literal::Symbol(
        ncl_compiler_front::SymbolRef::interned("COMMON-LISP", "INTEGER"),
    ));
    env.declare_type(y.clone(), specifier.clone());
    assert_eq!(env.variable_type(&y), Some(&specifier));
    assert!(!env.is_special(&y));

    env.pop_scope();
    assert!(env.lookup_variable(&x).is_none());
}

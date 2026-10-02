#![allow(clippy::unwrap_used, reason = "coverage tests assert on each result")]
#![allow(missing_docs)]

use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, Package, Runtime, ThreadContext, Word,
    make_array, make_cons, make_simple_vector, make_specialized_array, make_string,
};
use ncl_types::{
    ArrayDimension, ArrayDimensions, IntegerBound, NamedType, TypeError, TypeSpecifier, Value,
    parse_type_specifier, subtypep, typep,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_types::register(&runtime).unwrap();
    ncl_types::builtins::register(&runtime).unwrap();
    (runtime, ctx)
}

fn intern(ctx: &mut ThreadContext, runtime: &Runtime, package: &str, name: &str) -> Word {
    let package = runtime.find_package(ctx, package).unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Word {
    values.iter().rev().fold(Word::NIL, |tail, value| {
        make_cons(ctx, runtime, *value, tail).unwrap()
    })
}

fn builtin(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap()
}

fn options() -> ArrayOptions {
    ArrayOptions {
        element_type: ArrayElementType::T,
        initial_element: Word::NIL,
        adjustable: false,
        fill_pointer: None,
        displaced_to: None,
        displaced_index_offset: 0,
    }
}

#[test]
fn typep_checks_array_ranks_sizes_and_vector_kinds() {
    let (runtime, mut ctx) = setup();
    let vector =
        make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)]).unwrap();
    let matrix = make_array(&mut ctx, &runtime, &[2, 3], options()).unwrap();
    let bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0)],
    )
    .unwrap();
    let string = make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();

    assert!(typep(&mut ctx, vector, &TypeSpecifier::Named(NamedType::Vector)).unwrap());
    assert!(
        typep(
            &mut ctx,
            vector,
            &TypeSpecifier::Named(NamedType::SimpleArray)
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            vector,
            &TypeSpecifier::Vector {
                element_type: None,
                size: Some(ArrayDimension::Exact(2))
            }
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            vector,
            &TypeSpecifier::Vector {
                element_type: None,
                size: Some(ArrayDimension::Exact(3))
            }
        )
        .unwrap()
    );
    assert!(typep(&mut ctx, matrix, &TypeSpecifier::Named(NamedType::Array)).unwrap());
    assert!(!typep(&mut ctx, matrix, &TypeSpecifier::Named(NamedType::Vector)).unwrap());
    assert!(
        typep(
            &mut ctx,
            matrix,
            &TypeSpecifier::Array {
                element_type: None,
                dimensions: Some(ArrayDimensions::Rank(2)),
                simple: false
            }
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            matrix,
            &TypeSpecifier::Array {
                element_type: None,
                dimensions: Some(ArrayDimensions::Rank(1)),
                simple: false
            }
        )
        .unwrap()
    );
    assert!(typep(&mut ctx, bits, &TypeSpecifier::Named(NamedType::BitVector)).unwrap());
    assert!(
        typep(
            &mut ctx,
            string,
            &TypeSpecifier::Vector {
                element_type: Some(Box::new(TypeSpecifier::Named(NamedType::Character))),
                size: Some(ArrayDimension::Any)
            }
        )
        .unwrap()
    );
}

#[test]
fn typep_exercises_compound_short_circuits_cons_and_value_matching() {
    let (runtime, mut ctx) = setup();
    let cons = make_cons(&mut ctx, &runtime, Word::fixnum(7), Word::NIL).unwrap();
    let integer = TypeSpecifier::Named(NamedType::Integer);
    let character = TypeSpecifier::Named(NamedType::Character);
    assert!(
        typep(
            &mut ctx,
            Word::fixnum(7),
            &TypeSpecifier::Or(vec![
                integer.clone(),
                TypeSpecifier::Satisfies("never".into())
            ])
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(7),
            &TypeSpecifier::And(vec![
                character.clone(),
                TypeSpecifier::Satisfies("never".into())
            ])
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            Word::fixnum(7),
            &TypeSpecifier::Member(vec![Value::Integer(7), Value::Integer(8)])
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(7),
            &TypeSpecifier::Not(Box::new(integer))
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            cons,
            &TypeSpecifier::Cons {
                car: Box::new(TypeSpecifier::Named(NamedType::Integer)),
                cdr: Box::new(TypeSpecifier::Named(NamedType::Null))
            }
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::NIL,
            &TypeSpecifier::Cons {
                car: Box::new(TypeSpecifier::Named(NamedType::T)),
                cdr: Box::new(TypeSpecifier::Named(NamedType::T))
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            Word::character(65),
            &TypeSpecifier::Eql(Value::Character(65))
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::character(66),
            &TypeSpecifier::Eql(Value::Character(65))
        )
        .unwrap()
    );
    assert!(matches!(
        typep(&mut ctx, Word::NIL, &TypeSpecifier::Satisfies("P".into())),
        Err(TypeError::CannotInvoke(_))
    ));
}

#[test]
fn builtins_cover_keyword_type_of_subtypep_and_coercion_errors() {
    let (runtime, mut ctx) = setup();
    let keywordp = builtin(&runtime, &mut ctx, "KEYWORDP");
    let type_of = builtin(&runtime, &mut ctx, "TYPE-OF");
    let subtypep_builtin = builtin(&runtime, &mut ctx, "SUBTYPEP");
    let coerce = builtin(&runtime, &mut ctx, "COERCE");
    let keyword = intern(&mut ctx, &runtime, "KEYWORD", "WAVE11");
    let symbol = intern(&mut ctx, &runtime, "COMMON-LISP", "WAVE11");
    assert_eq!(
        runtime.call_builtin(&mut ctx, keywordp, &[keyword]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, keywordp, &[symbol]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, keywordp, &[Word::NIL]),
        Ok(Word::NIL)
    );
    let fixnum = intern(&mut ctx, &runtime, "COMMON-LISP", "FIXNUM");
    assert_eq!(
        runtime.call_builtin(&mut ctx, type_of, &[Word::fixnum(4)]),
        Ok(fixnum)
    );
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let number = intern(&mut ctx, &runtime, "COMMON-LISP", "NUMBER");
    assert_eq!(
        runtime.call_builtin(&mut ctx, subtypep_builtin, &[integer, number]),
        Ok(Word::TRUE)
    );
    assert_eq!(ctx.values(), &[Word::TRUE]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, subtypep_builtin, &[number, integer]),
        Ok(Word::NIL)
    );
    assert_eq!(ctx.values(), &[Word::NIL]);
    let bad = list(&mut ctx, &runtime, &[integer, Word::TRUE]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::fixnum(1), bad]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn parser_covers_optional_forms_bounds_and_invalid_shapes() {
    let (runtime, mut ctx) = setup();
    let simple_array = intern(&mut ctx, &runtime, "COMMON-LISP", "SIMPLE-ARRAY");
    let star = intern(&mut ctx, &runtime, "COMMON-LISP", "*");
    let simple_form = list(&mut ctx, &runtime, &[simple_array, star, star]);
    assert_eq!(
        parse_type_specifier(&mut ctx, simple_form).unwrap(),
        TypeSpecifier::Array {
            element_type: Some(Box::new(TypeSpecifier::Deftype {
                name: "*".into(),
                args: vec![]
            })),
            dimensions: Some(ArrayDimensions::Wild),
            simple: true
        }
    );
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let low = list(&mut ctx, &runtime, &[Word::fixnum(2)]);
    let range = list(&mut ctx, &runtime, &[integer, low, Word::fixnum(9)]);
    assert_eq!(
        parse_type_specifier(&mut ctx, range).unwrap(),
        TypeSpecifier::IntegerRange {
            low: IntegerBound::Exclusive(2),
            high: IntegerBound::Inclusive(9)
        }
    );
    let not = intern(&mut ctx, &runtime, "COMMON-LISP", "NOT");
    let malformed_not = list(&mut ctx, &runtime, &[not, Word::fixnum(1), Word::fixnum(2)]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, malformed_not),
        Err(TypeError::InvalidSpecifier(_))
    ));
    let function = intern(&mut ctx, &runtime, "COMMON-LISP", "FUNCTION");
    let malformed_function = list(&mut ctx, &runtime, &[function, Word::fixnum(1)]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, malformed_function),
        Err(TypeError::InvalidSpecifier(_))
    ));
    assert!(matches!(
        parse_type_specifier(&mut ctx, Word::fixnum(1)),
        Err(TypeError::InvalidSpecifier(_))
    ));
}

#[test]
fn subtypep_covers_named_tables_ranges_and_composite_results() {
    let named = |value| TypeSpecifier::Named(value);
    assert_eq!(
        subtypep(&named(NamedType::Fixnum), &named(NamedType::Integer)).unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(&named(NamedType::String), &named(NamedType::Sequence)).unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(&named(NamedType::Cons), &named(NamedType::Number)).unwrap(),
        (false, true)
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Exclusive(0),
                high: IntegerBound::Exclusive(10)
            },
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Inclusive(0),
                high: IntegerBound::Inclusive(10)
            }
        )
        .unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Inclusive(0),
                high: IntegerBound::Inclusive(10)
            },
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Exclusive(0),
                high: IntegerBound::Inclusive(10)
            }
        )
        .unwrap(),
        (false, false)
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::Or(vec![named(NamedType::Fixnum), named(NamedType::Bignum)]),
            &named(NamedType::Integer)
        )
        .unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(
            &named(NamedType::Fixnum),
            &TypeSpecifier::Or(vec![named(NamedType::String), named(NamedType::Integer)])
        )
        .unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::And(vec![named(NamedType::String), named(NamedType::Integer)]),
            &named(NamedType::Integer)
        )
        .unwrap(),
        (true, true)
    );
    assert!(matches!(
        subtypep(
            &TypeSpecifier::Deftype {
                name: "UNKNOWN".into(),
                args: vec![]
            },
            &named(NamedType::T)
        ),
        Err(TypeError::UnexpandedDeftype(_))
    ));
    assert!(matches!(
        subtypep(&named(NamedType::T), &TypeSpecifier::Satisfies("P".into())),
        Err(TypeError::CannotInvoke(_))
    ));
}

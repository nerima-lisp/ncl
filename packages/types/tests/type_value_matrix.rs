#![allow(clippy::unwrap_used, reason = "coverage tests assert on each result")]
#![allow(missing_docs)]

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, Package, Runtime, ThreadContext, Word,
    make_array, make_bignum_from_i128, make_complex, make_cons, make_double, make_ratio,
    make_simple_vector, make_specialized_array, make_string, make_structure,
};
use ncl_types::{
    ArrayDimension, ArrayDimensions, IntegerBound, NamedType, TypeError, TypeSpecifier,
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
fn typep_covers_remaining_numeric_and_negative_named_paths() {
    let (runtime, mut ctx) = setup();
    let bignum: Word = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 70)
        .unwrap()
        .into();
    let ratio: Word = make_ratio(&mut ctx, &runtime, Word::fixnum(7), Word::fixnum(3))
        .unwrap()
        .into();
    let float: Word = make_double(&mut ctx, &runtime, -3.5).unwrap().into();
    let complex: Word = make_complex(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2))
        .unwrap()
        .into();
    let symbol = intern(&mut ctx, &runtime, "COMMON-LISP", "WAVE13");
    let cons = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).unwrap();
    for (object, named) in [
        (bignum, NamedType::Number),
        (bignum, NamedType::Real),
        (bignum, NamedType::Rational),
        (ratio, NamedType::Rational),
        (float, NamedType::Number),
        (float, NamedType::Real),
        (float, NamedType::Float),
        (complex, NamedType::Number),
        (symbol, NamedType::Atom),
        (cons, NamedType::List),
    ] {
        assert!(typep(&mut ctx, object, &TypeSpecifier::Named(named)).unwrap());
    }
    for named in [
        NamedType::Nil,
        NamedType::ShortFloat,
        NamedType::SingleFloat,
        NamedType::LongFloat,
        NamedType::RandomState,
        NamedType::Restart,
        NamedType::ExtendedChar,
    ] {
        assert!(!typep(&mut ctx, Word::TRUE, &TypeSpecifier::Named(named)).unwrap());
    }
    assert!(typep(&mut ctx, Word::NIL, &TypeSpecifier::Named(NamedType::Atom)).unwrap());
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(1),
            &TypeSpecifier::Named(NamedType::Function)
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(1),
            &TypeSpecifier::Named(NamedType::Vector)
        )
        .unwrap()
    );
}

#[test]
fn typep_covers_array_dimension_forms_and_cons_failures() {
    let (runtime, mut ctx) = setup();
    let vector =
        make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)]).unwrap();
    let string = make_string(&mut ctx, &runtime, &['x', 'y']).unwrap();
    let matrix = make_array(&mut ctx, &runtime, &[2, 2], options()).unwrap();
    let bit = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(1), Word::fixnum(0)],
    )
    .unwrap();
    assert!(
        typep(
            &mut ctx,
            vector,
            &TypeSpecifier::Array {
                element_type: None,
                dimensions: Some(ArrayDimensions::Ranks(vec![ArrayDimension::Any])),
                simple: false
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            string,
            &TypeSpecifier::Array {
                element_type: None,
                dimensions: Some(ArrayDimensions::Rank(1)),
                simple: true
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            matrix,
            &TypeSpecifier::Array {
                element_type: None,
                dimensions: Some(ArrayDimensions::Ranks(vec![
                    ArrayDimension::Exact(2),
                    ArrayDimension::Any
                ])),
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
                dimensions: Some(ArrayDimensions::Ranks(vec![ArrayDimension::Exact(2)])),
                simple: false
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            bit,
            &TypeSpecifier::Named(NamedType::SimpleBitVector)
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            vector,
            &TypeSpecifier::Named(NamedType::BitVector)
        )
        .unwrap()
    );
    let improper = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2)).unwrap();
    let cons_spec = TypeSpecifier::Cons {
        car: Box::new(TypeSpecifier::Named(NamedType::Integer)),
        cdr: Box::new(TypeSpecifier::Named(NamedType::Null)),
    };
    assert!(!typep(&mut ctx, improper, &cons_spec).unwrap());
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
}

#[test]
fn builtins_cover_sequence_errors_and_type_of_special_objects() {
    let (runtime, mut ctx) = setup();
    let type_of = builtin(&runtime, &mut ctx, "TYPE-OF");
    let coerce = builtin(&runtime, &mut ctx, "COERCE");
    let string_type = intern(&mut ctx, &runtime, "COMMON-LISP", "STRING");
    let character_type = intern(&mut ctx, &runtime, "COMMON-LISP", "CHARACTER");
    let vector_type = intern(&mut ctx, &runtime, "COMMON-LISP", "VECTOR");
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::character(65)]).unwrap();
    let characters = list(
        &mut ctx,
        &runtime,
        &[Word::character(97), Word::character(98)],
    );
    let as_string = runtime
        .call_builtin(&mut ctx, coerce, &[vector, string_type])
        .unwrap();
    assert_eq!(ncl_object::string_length(&ctx, as_string).unwrap(), 1);
    let as_vector = runtime
        .call_builtin(&mut ctx, coerce, &[characters, vector_type])
        .unwrap();
    assert_eq!(
        ncl_object::simple_vector_length(&ctx, as_vector).unwrap(),
        2
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::fixnum(1), string_type]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::fixnum(1), character_type]),
        Err(ncl_object::ObjectError::TypeError)
    );
    let layout = runtime.register_structure_layout(0).unwrap();
    let structure = make_structure(&mut ctx, &runtime, layout, &[]).unwrap();
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Eq, Weakness::None)
        .unwrap()
        .as_word();
    for object in [structure, table] {
        let name = runtime.call_builtin(&mut ctx, type_of, &[object]).unwrap();
        assert!(matches!(
            ncl_object::classify_object(&ctx, name),
            ncl_object::ObjectRef::Symbol(_)
        ));
    }
    let empty_vector = runtime
        .call_builtin(&mut ctx, coerce, &[Word::NIL, vector_type])
        .unwrap();
    assert_eq!(
        ncl_object::simple_vector_length(&ctx, empty_vector).unwrap(),
        0
    );
}

#[test]
fn subtypep_covers_unbounded_bound_combinations_and_tables() {
    let named = |name| TypeSpecifier::Named(name);
    let ranges = [
        (
            IntegerBound::Unbounded,
            IntegerBound::Inclusive(4),
            IntegerBound::Inclusive(0),
            IntegerBound::Inclusive(5),
            (false, false),
        ),
        (
            IntegerBound::Inclusive(1),
            IntegerBound::Unbounded,
            IntegerBound::Inclusive(0),
            IntegerBound::Unbounded,
            (true, true),
        ),
        (
            IntegerBound::Exclusive(1),
            IntegerBound::Exclusive(4),
            IntegerBound::Exclusive(0),
            IntegerBound::Exclusive(5),
            (true, true),
        ),
        (
            IntegerBound::Inclusive(0),
            IntegerBound::Inclusive(5),
            IntegerBound::Exclusive(0),
            IntegerBound::Inclusive(5),
            (false, false),
        ),
    ];
    for (low, high, sup_low, sup_high, expected) in ranges {
        assert_eq!(
            subtypep(
                &TypeSpecifier::IntegerRange { low, high },
                &TypeSpecifier::IntegerRange {
                    low: sup_low,
                    high: sup_high
                }
            )
            .unwrap(),
            expected
        );
    }
    for (sub, sup) in [
        (NamedType::Fixnum, NamedType::Number),
        (NamedType::Bignum, NamedType::Rational),
        (NamedType::Ratio, NamedType::Real),
        (NamedType::DoubleFloat, NamedType::Float),
        (NamedType::SimpleVector, NamedType::Array),
        (NamedType::SimpleString, NamedType::Sequence),
        (NamedType::CompiledFunction, NamedType::Function),
        (NamedType::Keyword, NamedType::Symbol),
    ] {
        assert_eq!(subtypep(&named(sub), &named(sup)).unwrap(), (true, true));
    }
    assert_eq!(
        subtypep(&named(NamedType::Cons), &named(NamedType::Null)).unwrap(),
        (false, true)
    );
    assert_eq!(
        subtypep(&named(NamedType::Complex), &named(NamedType::Real)).unwrap(),
        (false, true)
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::Or(vec![named(NamedType::Integer), named(NamedType::String)]),
            &named(NamedType::Number)
        )
        .unwrap(),
        (false, false)
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
            &named(NamedType::String)
        )
        .unwrap(),
        (true, true)
    );
    assert!(matches!(
        subtypep(&TypeSpecifier::Satisfies("P".into()), &named(NamedType::T)),
        Err(TypeError::CannotInvoke(_))
    ));
}

#[test]
fn parser_covers_nested_specs_and_invalid_domain_forms() {
    let (runtime, mut ctx) = setup();
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let array = intern(&mut ctx, &runtime, "COMMON-LISP", "ARRAY");
    let vector = intern(&mut ctx, &runtime, "COMMON-LISP", "VECTOR");
    let function = intern(&mut ctx, &runtime, "COMMON-LISP", "FUNCTION");
    let star = intern(&mut ctx, &runtime, "COMMON-LISP", "*");
    let dimensions = list(&mut ctx, &runtime, &[star, Word::fixnum(4)]);
    let array_form = list(&mut ctx, &runtime, &[array, integer, dimensions]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, array_form).unwrap(),
        TypeSpecifier::Array { simple: false, .. }
    ));
    let vector_form = list(&mut ctx, &runtime, &[vector, integer, Word::fixnum(2)]);
    assert_eq!(
        parse_type_specifier(&mut ctx, vector_form).unwrap(),
        TypeSpecifier::Vector {
            element_type: Some(Box::new(TypeSpecifier::Named(NamedType::Integer))),
            size: Some(ArrayDimension::Exact(2))
        }
    );
    let lambda = list(&mut ctx, &runtime, &[integer]);
    let function_form = list(&mut ctx, &runtime, &[function, lambda, integer]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, function_form).unwrap(),
        TypeSpecifier::Function { .. }
    ));
    for malformed in [
        list(&mut ctx, &runtime, &[array, Word::fixnum(1)]),
        list(
            &mut ctx,
            &runtime,
            &[vector, Word::TRUE, Word::TRUE, Word::TRUE],
        ),
        list(&mut ctx, &runtime, &[function, Word::fixnum(1)]),
        list(
            &mut ctx,
            &runtime,
            &[integer, Word::TRUE, Word::TRUE, Word::TRUE],
        ),
    ] {
        assert!(matches!(
            parse_type_specifier(&mut ctx, malformed),
            Err(TypeError::InvalidSpecifier(_))
        ));
    }
}

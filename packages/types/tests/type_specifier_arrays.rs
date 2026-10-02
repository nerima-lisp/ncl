#![allow(clippy::unwrap_used, reason = "coverage tests assert on each result")]
#![allow(missing_docs)]

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, Package, Runtime, ThreadContext, Word,
    make_array, make_bignum_from_i128, make_complex, make_cons, make_double, make_ratio,
    make_simple_vector, make_specialized_array, make_stream, make_string, make_structure,
};
use ncl_types::{
    ArrayDimension::{Any as AnyDim, Exact as ExactDim, Exclusive as ExclusiveDim},
    ArrayDimensions,
    IntegerBound::{Exclusive as IExclusive, Inclusive, Unbounded},
    NamedType, TypeSpecifier, Value, parse_type_specifier, subtypep, typep,
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

fn cl(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    intern(ctx, runtime, "COMMON-LISP", name)
}

fn parse_form(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    items: &[Word],
) -> Result<TypeSpecifier, ncl_types::TypeError> {
    let form = list(ctx, runtime, items);
    parse_type_specifier(ctx, form)
}

fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Word {
    values.iter().rev().fold(Word::NIL, |tail, value| {
        make_cons(ctx, runtime, *value, tail).unwrap()
    })
}

fn spec(name: NamedType) -> TypeSpecifier {
    TypeSpecifier::Named(name)
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
fn parser_accepts_the_remaining_domain_forms() {
    let (runtime, mut ctx) = setup();
    let integer = cl(&mut ctx, &runtime, "INTEGER");
    let star = cl(&mut ctx, &runtime, "*");
    let string = cl(&mut ctx, &runtime, "STRING");
    let or = cl(&mut ctx, &runtime, "OR");
    let and = cl(&mut ctx, &runtime, "AND");
    let values = cl(&mut ctx, &runtime, "VALUES");
    let not = cl(&mut ctx, &runtime, "NOT");
    let member = cl(&mut ctx, &runtime, "MEMBER");
    let eql = cl(&mut ctx, &runtime, "EQL");
    let satisfies = cl(&mut ctx, &runtime, "SATISFIES");
    let cons = cl(&mut ctx, &runtime, "CONS");
    let array_op = cl(&mut ctx, &runtime, "ARRAY");
    let simple_array = cl(&mut ctx, &runtime, "SIMPLE-ARRAY");
    let vector = cl(&mut ctx, &runtime, "VECTOR");
    let function = cl(&mut ctx, &runtime, "FUNCTION");
    let deftype = cl(&mut ctx, &runtime, "MY-TYPE");
    let number = cl(&mut ctx, &runtime, "NUMBER");
    let pred = cl(&mut ctx, &runtime, "PRED");
    let exclusive_low = list(&mut ctx, &runtime, &[Word::fixnum(1)]);
    let exclusive_high = list(&mut ctx, &runtime, &[Word::fixnum(9)]);
    let dimensions = list(&mut ctx, &runtime, &[star, Word::fixnum(4)]);
    let lambda_list = list(&mut ctx, &runtime, &[integer]);

    let range = parse_form(
        &mut ctx,
        &runtime,
        &[integer, exclusive_low, exclusive_high],
    )
    .unwrap();
    assert_eq!(
        range,
        TypeSpecifier::IntegerRange {
            low: IExclusive(1),
            high: IExclusive(9)
        }
    );
    assert_eq!(
        parse_form(&mut ctx, &runtime, &[integer, star, star]).unwrap(),
        TypeSpecifier::IntegerRange {
            low: Unbounded,
            high: Unbounded
        }
    );

    let or = parse_form(&mut ctx, &runtime, &[or, integer, string]).unwrap();
    assert_eq!(
        or,
        TypeSpecifier::Or(vec![spec(NamedType::Integer), spec(NamedType::String)])
    );
    assert_eq!(
        parse_form(&mut ctx, &runtime, &[and, integer, number]).unwrap(),
        TypeSpecifier::And(vec![spec(NamedType::Integer), spec(NamedType::Number)])
    );
    assert_eq!(
        parse_form(&mut ctx, &runtime, &[values, integer, string]).unwrap(),
        TypeSpecifier::Values(vec![spec(NamedType::Integer), spec(NamedType::String)])
    );
    assert_eq!(
        parse_form(&mut ctx, &runtime, &[not, integer]).unwrap(),
        TypeSpecifier::Not(Box::new(spec(NamedType::Integer)))
    );

    let x_string = make_string(&mut ctx, &runtime, &['x']).unwrap();
    let member = parse_form(
        &mut ctx,
        &runtime,
        &[member, Word::fixnum(3), x_string, Word::character(122)],
    )
    .unwrap();
    assert_eq!(
        member,
        TypeSpecifier::Member(vec![
            Value::Integer(3),
            Value::String("x".to_owned()),
            Value::Character(122),
        ])
    );
    assert_eq!(
        parse_form(&mut ctx, &runtime, &[eql, Word::TRUE]).unwrap(),
        TypeSpecifier::Eql(Value::True)
    );
    assert_eq!(
        parse_form(&mut ctx, &runtime, &[satisfies, pred]).unwrap(),
        TypeSpecifier::Satisfies("PRED".to_owned())
    );

    assert_eq!(
        parse_form(&mut ctx, &runtime, &[cons]).unwrap(),
        TypeSpecifier::Cons {
            car: Box::new(spec(NamedType::T)),
            cdr: Box::new(spec(NamedType::T))
        }
    );
    let array = parse_form(&mut ctx, &runtime, &[array_op, integer, dimensions]).unwrap();
    assert_eq!(
        array,
        TypeSpecifier::Array {
            element_type: Some(Box::new(spec(NamedType::Integer))),
            dimensions: Some(ArrayDimensions::Ranks(vec![AnyDim, ExactDim(4)])),
            simple: false
        }
    );
    assert_eq!(
        parse_form(&mut ctx, &runtime, &[simple_array, star, Word::fixnum(2)]).unwrap(),
        TypeSpecifier::Array {
            element_type: Some(Box::new(TypeSpecifier::Deftype {
                name: "*".to_owned(),
                args: vec![]
            })),
            dimensions: Some(ArrayDimensions::Rank(2)),
            simple: true
        }
    );
    assert_eq!(
        parse_form(&mut ctx, &runtime, &[vector, integer, star]).unwrap(),
        TypeSpecifier::Vector {
            element_type: Some(Box::new(spec(NamedType::Integer))),
            size: Some(AnyDim)
        }
    );
    assert_eq!(
        parse_form(&mut ctx, &runtime, &[function, lambda_list, string]).unwrap(),
        TypeSpecifier::Function {
            lambda_list: vec![spec(NamedType::Integer)],
            return_type: Box::new(spec(NamedType::String))
        }
    );
    assert_eq!(
        parse_form(&mut ctx, &runtime, &[deftype, Word::fixnum(8), Word::TRUE]).unwrap(),
        TypeSpecifier::Deftype {
            name: "MY-TYPE".to_owned(),
            args: vec![Value::Integer(8), Value::True]
        }
    );
}

#[test]
fn parser_rejects_invalid_arity_bounds_and_forms() {
    let (runtime, mut ctx) = setup();
    let integer = cl(&mut ctx, &runtime, "INTEGER");
    let eql = cl(&mut ctx, &runtime, "EQL");
    let satisfies = cl(&mut ctx, &runtime, "SATISFIES");
    let cons = cl(&mut ctx, &runtime, "CONS");
    let list_type = cl(&mut ctx, &runtime, "LIST");
    let string = cl(&mut ctx, &runtime, "STRING");
    let vector = cl(&mut ctx, &runtime, "VECTOR");
    let array = cl(&mut ctx, &runtime, "ARRAY");
    let member = cl(&mut ctx, &runtime, "MEMBER");
    let function_type = cl(&mut ctx, &runtime, "FUNCTION");
    let negative_dimension = list(&mut ctx, &runtime, &[Word::fixnum(-1)]);
    let member_form = list(&mut ctx, &runtime, &[Word::fixnum(1)]);
    let invalid = [
        list(&mut ctx, &runtime, &[integer, Word::TRUE]),
        list(&mut ctx, &runtime, &[eql]),
        list(&mut ctx, &runtime, &[eql, Word::fixnum(1), Word::fixnum(2)]),
        list(&mut ctx, &runtime, &[satisfies, Word::fixnum(1)]),
        list(&mut ctx, &runtime, &[cons, integer, list_type, string]),
        list(&mut ctx, &runtime, &[vector, integer, Word::fixnum(-1)]),
        list(&mut ctx, &runtime, &[function_type, Word::fixnum(1)]),
        list(&mut ctx, &runtime, &[array, integer, Word::fixnum(-1)]),
        list(&mut ctx, &runtime, &[array, integer, negative_dimension]),
        list(&mut ctx, &runtime, &[member, member_form]),
    ];
    for value in invalid {
        assert!(matches!(
            parse_type_specifier(&mut ctx, value),
            Err(ncl_types::TypeError::InvalidSpecifier(_))
        ));
    }
}

#[test]
fn subtypep_covers_named_hierarchy_disjointness_ranges_and_combinations() {
    let named_pairs = [
        (NamedType::Fixnum, NamedType::Integer),
        (NamedType::Bignum, NamedType::Integer),
        (NamedType::Integer, NamedType::Rational),
        (NamedType::Rational, NamedType::Real),
        (NamedType::Real, NamedType::Number),
        (NamedType::DoubleFloat, NamedType::Float),
        (NamedType::String, NamedType::Sequence),
        (NamedType::SimpleString, NamedType::String),
        (NamedType::Cons, NamedType::List),
        (NamedType::Null, NamedType::List),
        (NamedType::Vector, NamedType::Array),
        (NamedType::SimpleVector, NamedType::Vector),
        (NamedType::Keyword, NamedType::Symbol),
        (NamedType::CompiledFunction, NamedType::Function),
    ];
    for (sub, sup) in named_pairs {
        assert_eq!(subtypep(&spec(sub), &spec(sup)).unwrap(), (true, true));
    }
    for (sub, sup) in [
        (NamedType::Integer, NamedType::Float),
        (NamedType::Float, NamedType::Integer),
        (NamedType::Cons, NamedType::Null),
        (NamedType::Number, NamedType::Symbol),
        (NamedType::Ratio, NamedType::Fixnum),
        (NamedType::Complex, NamedType::Real),
    ] {
        assert_eq!(subtypep(&spec(sub), &spec(sup)).unwrap(), (false, true));
    }
    assert_eq!(
        subtypep(&spec(NamedType::String), &spec(NamedType::Integer)).unwrap(),
        (false, false)
    );

    let ranges = |low, high| TypeSpecifier::IntegerRange { low, high };
    for (sub, sup, expected) in [
        (
            ranges(Inclusive(2), Inclusive(8)),
            ranges(Inclusive(0), Inclusive(10)),
            (true, true),
        ),
        (
            ranges(IExclusive(2), IExclusive(8)),
            ranges(Inclusive(2), Inclusive(8)),
            (true, true),
        ),
        (
            ranges(Inclusive(0), Unbounded),
            ranges(Inclusive(0), Unbounded),
            (true, true),
        ),
        (
            ranges(Unbounded, Inclusive(10)),
            ranges(Inclusive(0), Inclusive(10)),
            (false, false),
        ),
        (
            ranges(Inclusive(0), Inclusive(10)),
            ranges(IExclusive(0), Inclusive(10)),
            (false, false),
        ),
        (
            ranges(Inclusive(0), IExclusive(10)),
            ranges(Inclusive(0), IExclusive(10)),
            (true, true),
        ),
    ] {
        assert_eq!(subtypep(&sub, &sup).unwrap(), expected);
    }

    let integer = spec(NamedType::Integer);
    let number = spec(NamedType::Number);
    let string = spec(NamedType::String);
    assert_eq!(
        subtypep(
            &TypeSpecifier::Or(vec![integer.clone(), number.clone()]),
            &number
        )
        .unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::Or(vec![string.clone(), integer.clone()]),
            &number
        )
        .unwrap(),
        (false, false)
    );
    assert_eq!(
        subtypep(
            &integer,
            &TypeSpecifier::Or(vec![string.clone(), number.clone()])
        )
        .unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(&integer, &TypeSpecifier::Or(vec![string.clone()])).unwrap(),
        (false, false)
    );
    assert_eq!(
        subtypep(&TypeSpecifier::And(vec![string, integer.clone()]), &number).unwrap(),
        (true, true)
    );
    assert!(matches!(
        subtypep(
            &TypeSpecifier::Deftype {
                name: "X".to_owned(),
                args: vec![]
            },
            &number
        ),
        Err(ncl_types::TypeError::UnexpandedDeftype(_))
    ));
    assert!(matches!(
        subtypep(&integer, &TypeSpecifier::Satisfies("P".to_owned())),
        Err(ncl_types::TypeError::CannotInvoke(_))
    ));
}

#[test]
fn typep_covers_named_objects_and_composite_boundaries() {
    let (runtime, mut ctx) = setup();
    let symbol = intern(&mut ctx, &runtime, "COMMON-LISP", "X");
    let keyword = intern(&mut ctx, &runtime, "KEYWORD", "X");
    let package = runtime.find_package(&ctx, "COMMON-LISP").unwrap();
    let function = runtime
        .function(&mut ctx, "COMMON-LISP", "TYPE-OF")
        .unwrap();
    let big: Word = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 70)
        .unwrap()
        .into();
    let ratio: Word = make_ratio(&mut ctx, &runtime, Word::fixnum(3), Word::fixnum(2))
        .unwrap()
        .into();
    let double: Word = make_double(&mut ctx, &runtime, 2.5).unwrap().into();
    let complex: Word = make_complex(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2))
        .unwrap()
        .into();
    let string = make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    let vector =
        make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)]).unwrap();
    let bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0), Word::fixnum(1)],
    )
    .unwrap();
    let array = make_array(&mut ctx, &runtime, &[2, 3], options()).unwrap();
    let cons = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).unwrap();
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Eq, Weakness::None)
        .unwrap()
        .as_word();
    let stream = make_stream(
        &mut ctx,
        &runtime,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap()
    .into();
    let layout = runtime.register_structure_layout(0).unwrap();
    let structure = make_structure(&mut ctx, &runtime, layout, &[]).unwrap();

    let cases = [
        (Word::TRUE, NamedType::T, true),
        (Word::NIL, NamedType::Nil, false),
        (Word::NIL, NamedType::Boolean, true),
        (symbol, NamedType::Symbol, true),
        (keyword, NamedType::Keyword, true),
        (package, NamedType::Package, true),
        (function, NamedType::Function, true),
        (function, NamedType::CompiledFunction, true),
        (cons, NamedType::Cons, true),
        (cons, NamedType::List, true),
        (Word::NIL, NamedType::Null, true),
        (Word::fixnum(1), NamedType::Atom, true),
        (Word::fixnum(1), NamedType::Number, true),
        (Word::fixnum(1), NamedType::Real, true),
        (Word::fixnum(1), NamedType::Rational, true),
        (Word::fixnum(1), NamedType::Integer, true),
        (Word::fixnum(1), NamedType::Fixnum, true),
        (big, NamedType::Bignum, true),
        (ratio, NamedType::Ratio, true),
        (double, NamedType::Float, true),
        (double, NamedType::DoubleFloat, true),
        (complex, NamedType::Complex, true),
        (Word::character(65), NamedType::Character, true),
        (Word::character(65), NamedType::StandardChar, true),
        (string, NamedType::String, true),
        (string, NamedType::SimpleString, true),
        (bits, NamedType::BitVector, true),
        (bits, NamedType::SimpleBitVector, true),
        (vector, NamedType::Vector, true),
        (vector, NamedType::SimpleVector, true),
        (array, NamedType::Array, true),
        (table, NamedType::HashTable, true),
        (stream, NamedType::Stream, true),
        (structure, NamedType::Structure, true),
        (Word::fixnum(1), NamedType::ShortFloat, false),
        (Word::fixnum(1), NamedType::ExtendedChar, false),
        (Word::fixnum(1), NamedType::RandomState, false),
        (Word::fixnum(1), NamedType::Restart, false),
    ];
    for (object, named, expected) in cases {
        assert_eq!(
            typep(&mut ctx, object, &spec(named)).unwrap(),
            expected,
            "{named:?}"
        );
    }
    assert!(typep(&mut ctx, Word::fixnum(1), &TypeSpecifier::Values(vec![])).unwrap());
    assert!(
        typep(
            &mut ctx,
            function,
            &TypeSpecifier::Function {
                lambda_list: vec![],
                return_type: Box::new(spec(NamedType::T))
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            cons,
            &TypeSpecifier::Cons {
                car: Box::new(spec(NamedType::Integer)),
                cdr: Box::new(spec(NamedType::Null))
            }
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            cons,
            &TypeSpecifier::Cons {
                car: Box::new(spec(NamedType::String)),
                cdr: Box::new(spec(NamedType::Null))
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            Word::fixnum(4),
            &TypeSpecifier::IntegerRange {
                low: IExclusive(3),
                high: IExclusive(5)
            }
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(3),
            &TypeSpecifier::IntegerRange {
                low: IExclusive(3),
                high: Unbounded
            }
        )
        .unwrap()
    );
    let opaque = Value::Opaque(vector.bits());
    assert!(typep(&mut ctx, vector, &TypeSpecifier::Eql(opaque)).unwrap());
    assert!(
        typep(
            &mut ctx,
            Word::fixnum(1),
            &TypeSpecifier::Or(vec![spec(NamedType::String), spec(NamedType::Integer)])
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(1),
            &TypeSpecifier::And(vec![spec(NamedType::String), spec(NamedType::Integer)])
        )
        .unwrap()
    );
}

#[test]
fn typep_checks_all_dimension_forms_and_vector_lengths() {
    let (runtime, mut ctx) = setup();
    let vector =
        make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)]).unwrap();
    let array = make_array(&mut ctx, &runtime, &[2, 3], options()).unwrap();
    let rank_one = make_array(&mut ctx, &runtime, &[2], options()).unwrap();
    let string = make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    let wild = |simple| TypeSpecifier::Array {
        element_type: None,
        dimensions: Some(ArrayDimensions::Wild),
        simple,
    };
    assert!(typep(&mut ctx, vector, &wild(true)).unwrap());
    assert!(typep(&mut ctx, array, &wild(false)).unwrap());
    assert!(!typep(&mut ctx, array, &wild(true)).unwrap());
    assert!(
        typep(
            &mut ctx,
            array,
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
            array,
            &TypeSpecifier::Array {
                element_type: None,
                dimensions: Some(ArrayDimensions::Rank(1)),
                simple: false
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            array,
            &TypeSpecifier::Array {
                element_type: None,
                dimensions: Some(ArrayDimensions::Ranks(vec![AnyDim, ExclusiveDim(4)])),
                simple: false
            }
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            array,
            &TypeSpecifier::Array {
                element_type: None,
                dimensions: Some(ArrayDimensions::Ranks(vec![ExactDim(2)])),
                simple: false
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            vector,
            &TypeSpecifier::Vector {
                element_type: None,
                size: Some(AnyDim)
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            vector,
            &TypeSpecifier::Vector {
                element_type: None,
                size: Some(ExclusiveDim(3))
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
                size: Some(ExclusiveDim(2))
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            rank_one,
            &TypeSpecifier::Vector {
                element_type: None,
                size: Some(ExactDim(2))
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            string,
            &TypeSpecifier::Vector {
                element_type: None,
                size: Some(ExactDim(2))
            }
        )
        .unwrap()
    );
}

#[test]
fn extra_builtin_paths_cover_empty_sequences_and_function_errors() {
    let (runtime, mut ctx) = setup();
    let mut function = |name| {
        FunctionObject::try_from(runtime.function(&mut ctx, "COMMON-LISP", name).unwrap()).unwrap()
    };
    let coerce = function("COERCE");
    let list_type = intern(&mut ctx, &runtime, "COMMON-LISP", "LIST");
    let vector_type = intern(&mut ctx, &runtime, "COMMON-LISP", "VECTOR");
    let string_type = intern(&mut ctx, &runtime, "COMMON-LISP", "STRING");
    let function_type = intern(&mut ctx, &runtime, "COMMON-LISP", "FUNCTION");
    let empty_vector = make_simple_vector(&mut ctx, &runtime, &[]).unwrap();
    let empty_string = make_string(&mut ctx, &runtime, &[]).unwrap();
    let empty_list = runtime
        .call_builtin(&mut ctx, coerce, &[Word::NIL, list_type])
        .unwrap();
    assert_eq!(empty_list, Word::NIL);
    let list_from_empty_vector = runtime
        .call_builtin(&mut ctx, coerce, &[empty_vector, list_type])
        .unwrap();
    assert_eq!(list_from_empty_vector, Word::NIL);
    let vector_from_empty_list = runtime
        .call_builtin(&mut ctx, coerce, &[Word::NIL, vector_type])
        .unwrap();
    assert_eq!(
        ncl_object::simple_vector_length(&ctx, vector_from_empty_list).unwrap(),
        0
    );
    let string_from_empty_list = runtime
        .call_builtin(&mut ctx, coerce, &[Word::NIL, string_type])
        .unwrap();
    assert_eq!(
        ncl_object::string_length(&ctx, string_from_empty_list).unwrap(),
        0
    );
    let function_symbol = intern(&mut ctx, &runtime, "COMMON-LISP", "NO-SUCH-FUNCTION");
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[function_symbol, function_type]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::NIL, function_type]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, coerce, &[empty_string, list_type])
            .unwrap(),
        Word::NIL
    );
}

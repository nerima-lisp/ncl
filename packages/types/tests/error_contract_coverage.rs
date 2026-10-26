#![allow(clippy::unwrap_used, reason = "tests assert on type-system behavior")]

//! Behavioral checks for type-system errors, membership, and derived types.

use std::error::Error;

use ncl_object::{
    ObjectError, Package, Runtime, ThreadContext, Word, make_bignum_from_i128,
    make_bignum_from_limbs, make_cons, make_double, make_string, set_symbol_plist,
};
use ncl_types::{
    DeftypeExpanderCaller, IntegerBound, NamedType, TypeError, TypeSpecifier, Value,
    expand_deftype_specifier_with, parse_type_specifier, typep,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    (runtime, ctx)
}

fn intern(ctx: &mut ThreadContext, runtime: &Runtime, package_name: &str, name: &str) -> Word {
    let package = runtime.find_package(ctx, package_name).unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn list(ctx: &mut ThreadContext, runtime: &Runtime, items: &[Word]) -> Word {
    let mut result = Word::NIL;
    for &item in items.iter().rev() {
        result = make_cons(ctx, runtime, item, result).unwrap();
    }
    result
}

fn install_expander(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    function: Word,
) -> Word {
    let symbol = intern(ctx, runtime, "COMMON-LISP", name);
    let package = runtime.ensure_package(ctx, "NCL").unwrap();
    let property = Package::from_word(package)
        .intern(ctx, runtime, "DEFTYPE-EXPANDER")
        .unwrap()
        .0;
    let plist = list(ctx, runtime, &[property, function]);
    set_symbol_plist(ctx, symbol, plist).unwrap();
    symbol
}

struct MappingExpander {
    expansions: Vec<(Word, Word)>,
    calls: Vec<(Word, Word)>,
}

impl DeftypeExpanderCaller for MappingExpander {
    fn call_expander(
        &mut self,
        _ctx: &mut ThreadContext,
        _runtime: &Runtime,
        function: Word,
        whole_form: Word,
    ) -> Result<Word, ObjectError> {
        self.calls.push((function, whole_form));
        self.expansions
            .iter()
            .find_map(|(registered, expansion)| (*registered == function).then_some(*expansion))
            .ok_or(ObjectError::UndefinedFunction)
    }
}

#[test]
fn type_error_display_source_and_conversion_contract() {
    let invalid_specifier = TypeError::InvalidSpecifier(Word::fixnum(1));
    let invalid_form = TypeError::InvalidForm;
    let object_error = TypeError::Object(ObjectError::TypeError);
    let cannot_invoke = TypeError::CannotInvoke("EVENP".to_owned());
    let unexpanded_deftype = TypeError::UnexpandedDeftype("POSITIVE-INTEGER".to_owned());
    let cannot_serialize = TypeError::CannotSerialize;

    assert_eq!(
        invalid_specifier.to_string(),
        "invalid type specifier: Word(0x2)"
    );
    assert_eq!(invalid_form.to_string(), "invalid type specifier form");
    assert_eq!(object_error.to_string(), "object error: TypeError");
    assert_eq!(cannot_invoke.to_string(), "cannot invoke predicate: EVENP");
    assert_eq!(
        unexpanded_deftype.to_string(),
        "unexpanded deftype: POSITIVE-INTEGER"
    );
    assert_eq!(
        cannot_serialize.to_string(),
        "cannot serialize type-specifier value"
    );

    assert!(Error::source(&invalid_specifier).is_none());
    assert!(Error::source(&invalid_form).is_none());
    assert_eq!(
        Error::source(&object_error).map(ToString::to_string),
        Some("TypeError".to_owned())
    );
    assert!(Error::source(&cannot_invoke).is_none());
    assert!(Error::source(&unexpanded_deftype).is_none());
    assert!(Error::source(&cannot_serialize).is_none());

    assert_eq!(
        TypeError::from(ObjectError::TypeError),
        TypeError::Object(ObjectError::TypeError)
    );
    assert_eq!(
        ObjectError::from(TypeError::CannotSerialize),
        ObjectError::TypeError
    );
}

#[test]
fn parsed_integer_zero_to_unbounded_includes_bignums() {
    let (runtime, mut ctx) = setup();
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let star = intern(&mut ctx, &runtime, "COMMON-LISP", "*");
    let form = list(&mut ctx, &runtime, &[integer, Word::fixnum(0), star]);
    let specifier = parse_type_specifier(&mut ctx, form).unwrap();

    assert_eq!(
        specifier,
        TypeSpecifier::IntegerRange {
            low: IntegerBound::Inclusive(0),
            high: IntegerBound::Unbounded,
        }
    );

    let large_positive = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 70)
        .unwrap()
        .into();
    let large_negative = make_bignum_from_i128(&mut ctx, &runtime, -(1_i128 << 70))
        .unwrap()
        .into();
    let zero_bignum = make_bignum_from_i128(&mut ctx, &runtime, 0).unwrap().into();
    let small_bignum = make_bignum_from_limbs(&mut ctx, &runtime, false, &[5])
        .unwrap()
        .into();
    let positive_boundary = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 62)
        .unwrap()
        .into();
    let negative_boundary = make_bignum_from_i128(&mut ctx, &runtime, -(1_i128 << 62))
        .unwrap()
        .into();
    let bounded = TypeSpecifier::IntegerRange {
        low: IntegerBound::Inclusive(0),
        high: IntegerBound::Inclusive(i64::MAX),
    };
    let negative_only = TypeSpecifier::IntegerRange {
        low: IntegerBound::Unbounded,
        high: IntegerBound::Inclusive(-1),
    };
    let exact_positive = TypeSpecifier::IntegerRange {
        low: IntegerBound::Inclusive(1_i64 << 62),
        high: IntegerBound::Inclusive(1_i64 << 62),
    };
    let exact_negative = TypeSpecifier::IntegerRange {
        low: IntegerBound::Inclusive(-(1_i64 << 62)),
        high: IntegerBound::Inclusive(-(1_i64 << 62)),
    };
    let exclusive_lower = TypeSpecifier::IntegerRange {
        low: IntegerBound::Exclusive(1_i64 << 62),
        high: IntegerBound::Unbounded,
    };
    let exclusive_upper = TypeSpecifier::IntegerRange {
        low: IntegerBound::Unbounded,
        high: IntegerBound::Exclusive(1_i64 << 62),
    };
    let negative_upper = TypeSpecifier::IntegerRange {
        low: IntegerBound::Unbounded,
        high: IntegerBound::Inclusive(-(1_i64 << 62)),
    };

    assert!(typep(&mut ctx, Word::fixnum(0), &specifier).unwrap());
    assert!(typep(&mut ctx, large_positive, &specifier).unwrap());
    assert!(!typep(&mut ctx, Word::fixnum(-1), &specifier).unwrap());
    assert!(!typep(&mut ctx, large_negative, &specifier).unwrap());
    assert!(typep(&mut ctx, zero_bignum, &specifier).unwrap());
    assert!(typep(&mut ctx, small_bignum, &bounded).unwrap());
    assert!(!typep(&mut ctx, large_positive, &bounded).unwrap());
    assert!(!typep(&mut ctx, large_positive, &negative_only).unwrap());
    assert!(typep(&mut ctx, positive_boundary, &exact_positive).unwrap());
    assert!(typep(&mut ctx, negative_boundary, &exact_negative).unwrap());
    assert!(!typep(&mut ctx, positive_boundary, &exclusive_lower).unwrap());
    assert!(!typep(&mut ctx, positive_boundary, &exclusive_upper).unwrap());
    assert!(typep(&mut ctx, large_negative, &negative_upper).unwrap());
}

#[test]
fn compound_type_membership_matches_clhs_sets() {
    let (runtime, mut ctx) = setup();
    let or = intern(&mut ctx, &runtime, "COMMON-LISP", "OR");
    let null = intern(&mut ctx, &runtime, "COMMON-LISP", "NULL");
    let and = intern(&mut ctx, &runtime, "COMMON-LISP", "AND");
    let not = intern(&mut ctx, &runtime, "COMMON-LISP", "NOT");
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let number = intern(&mut ctx, &runtime, "COMMON-LISP", "NUMBER");
    let member = intern(&mut ctx, &runtime, "COMMON-LISP", "MEMBER");
    let star = intern(&mut ctx, &runtime, "COMMON-LISP", "*");
    let nonnegative_integer = list(&mut ctx, &runtime, &[integer, Word::fixnum(0), star]);

    let or_form = list(&mut ctx, &runtime, &[or, null, nonnegative_integer]);
    let or_specifier = parse_type_specifier(&mut ctx, or_form).unwrap();
    assert!(typep(&mut ctx, Word::NIL, &or_specifier).unwrap());
    assert!(typep(&mut ctx, Word::fixnum(4), &or_specifier).unwrap());
    assert!(!typep(&mut ctx, Word::fixnum(-1), &or_specifier).unwrap());

    let and_form = list(&mut ctx, &runtime, &[and, number, integer]);
    let and_specifier = parse_type_specifier(&mut ctx, and_form).unwrap();
    let one = Word::fixnum(1);
    let one_point_zero = make_double(&mut ctx, &runtime, 1.0).unwrap().into();
    assert!(typep(&mut ctx, one, &and_specifier).unwrap());
    assert!(!typep(&mut ctx, one_point_zero, &and_specifier).unwrap());

    let not_form = list(&mut ctx, &runtime, &[not, integer]);
    let not_specifier = parse_type_specifier(&mut ctx, not_form).unwrap();
    let string = make_string(&mut ctx, &runtime, &['x']).unwrap();
    assert!(typep(&mut ctx, string, &not_specifier).unwrap());
    assert!(!typep(&mut ctx, one, &not_specifier).unwrap());

    let member_form = list(
        &mut ctx,
        &runtime,
        &[member, Word::fixnum(1), Word::fixnum(2)],
    );
    let member_specifier = parse_type_specifier(&mut ctx, member_form).unwrap();
    assert!(typep(&mut ctx, one, &member_specifier).unwrap());
    assert!(!typep(&mut ctx, Word::fixnum(3), &member_specifier).unwrap());
    assert!(!typep(&mut ctx, one_point_zero, &member_specifier).unwrap());
}

#[test]
fn deftype_expansion_recurses_through_cons_type_arguments() {
    let (runtime, mut ctx) = setup();
    let inner_function = Word::fixnum(41);
    let base_function = Word::fixnum(42);
    let inner = install_expander(&mut ctx, &runtime, "COV-T-INNER", inner_function);
    let base = install_expander(&mut ctx, &runtime, "COV-T-BASE", base_function);
    let or = intern(&mut ctx, &runtime, "COMMON-LISP", "OR");
    let null = intern(&mut ctx, &runtime, "COMMON-LISP", "NULL");
    let cons = intern(&mut ctx, &runtime, "COMMON-LISP", "CONS");
    let member = intern(&mut ctx, &runtime, "COMMON-LISP", "MEMBER");
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let star = intern(&mut ctx, &runtime, "COMMON-LISP", "*");
    let base_form = list(&mut ctx, &runtime, &[base]);
    let inner_form = list(&mut ctx, &runtime, &[inner]);
    let inner_expansion = list(&mut ctx, &runtime, &[or, null, base_form]);
    let base_expansion = list(&mut ctx, &runtime, &[integer, Word::fixnum(0), star]);
    let member_form = list(&mut ctx, &runtime, &[member, Word::fixnum(7)]);
    let form = list(&mut ctx, &runtime, &[cons, inner_form, member_form]);
    let mut caller = MappingExpander {
        expansions: vec![
            (inner_function, inner_expansion),
            (base_function, base_expansion),
        ],
        calls: Vec::new(),
    };

    let expanded = expand_deftype_specifier_with(&mut ctx, &runtime, form, &mut caller).unwrap();
    let specifier = parse_type_specifier(&mut ctx, expanded).unwrap();
    assert_eq!(
        specifier,
        TypeSpecifier::Cons {
            car: Box::new(TypeSpecifier::Or(vec![
                TypeSpecifier::Named(NamedType::Null),
                TypeSpecifier::IntegerRange {
                    low: IntegerBound::Inclusive(0),
                    high: IntegerBound::Unbounded,
                },
            ])),
            cdr: Box::new(TypeSpecifier::Member(vec![Value::Integer(7)])),
        }
    );
    assert_eq!(
        caller.calls,
        vec![(inner_function, inner_form), (base_function, base_form)]
    );

    let nil_tail = make_cons(&mut ctx, &runtime, Word::NIL, Word::fixnum(7)).unwrap();
    let positive_tail = make_cons(&mut ctx, &runtime, Word::fixnum(5), Word::fixnum(7)).unwrap();
    let negative_tail = make_cons(&mut ctx, &runtime, Word::fixnum(-1), Word::fixnum(7)).unwrap();
    let wrong_member = make_cons(&mut ctx, &runtime, Word::fixnum(5), Word::fixnum(8)).unwrap();

    assert!(typep(&mut ctx, nil_tail, &specifier).unwrap());
    assert!(typep(&mut ctx, positive_tail, &specifier).unwrap());
    assert!(!typep(&mut ctx, negative_tail, &specifier).unwrap());
    assert!(!typep(&mut ctx, wrong_member, &specifier).unwrap());
}

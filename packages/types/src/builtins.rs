//! Rust builtins for `COERCE`, `KEYWORDP`, `TYPE-OF`, and `SUBTYPEP`.
//!
//! These functions were previously declared (`define_function`) but left
//! unbound (`Word::UNBOUND`); this module gives them real implementations and
//! [`register`] binds them into the `COMMON-LISP` package.

use ncl_object::{
    Arity, Bignum, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier,
    BuiltinImplementation, BuiltinName, BuiltinPackage, DoubleFloat, Local, MultipleValues,
    ObjectError, ObjectRef, Package, Parameter, ParameterType, Ratio, Runtime, Scope,
    ThreadContext, Word, bignum_limbs, bignum_sign, car, cdr, classify_object, double_value,
    make_double, make_simple_vector, make_string, ratio_denominator, ratio_numerator,
    simple_vector_length, simple_vector_ref, string_length, string_ref, symbol_function,
};

use crate::{NamedType, TypeSpecifier, parse_type_specifier, subtypep, typep};

const OBJECT: Parameter = Parameter {
    name: BuiltinName::new("object"),
    ty: ParameterType::Any,
};
const TWO_OBJECTS: &[Parameter] = &[OBJECT, OBJECT];

fn bool_word(value: bool) -> Word {
    if value { Word::TRUE } else { Word::NIL }
}

/// Register `COERCE`, `KEYWORDP`, `TYPE-OF`, and `SUBTYPEP` as real Rust
/// builtins, overriding the `Word::UNBOUND` placeholders installed by
/// [`crate::register::register`].
///
/// # Errors
///
/// Propagates package, intern, and registration failures.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("KEYWORDP")),
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: ncl_object::LambdaList::fixed(&[OBJECT]),
                convention: BuiltinConvention::Direct(Arity::exact(1)),
            },
            keywordp_builtin,
        ),
    )?;
    runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("TYPE-OF")),
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: ncl_object::LambdaList::fixed(&[OBJECT]),
                convention: BuiltinConvention::Direct(Arity::exact(1)),
            },
            type_of_builtin,
        ),
    )?;
    runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("COERCE")),
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: ncl_object::LambdaList::fixed(TWO_OBJECTS),
                convention: BuiltinConvention::Direct(Arity::exact(2)),
            },
            coerce_builtin,
        ),
    )?;
    runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("SUBTYPEP")),
        BuiltinImplementation::adapted(
            Builtin {
                lambda_list: ncl_object::LambdaList::with_optional(TWO_OBJECTS, &[OBJECT]),
                convention: BuiltinConvention::Adapted,
            },
            subtypep_builtin,
            |args| Ok(args.as_slice().to_vec()),
        ),
    )?;
    Ok(())
}

fn intern_common_lisp(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<Word, ObjectError> {
    let package = runtime
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    Ok(Package::from_word(package).intern(ctx, runtime, name)?.0)
}

fn keywordp_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let object = args.required(0)?;
    let result = crate::typep::is_keyword(ctx, object).map_err(|_| ObjectError::TypeError)?;
    Ok(bool_word(result))
}

#[allow(clippy::wildcard_enum_match_arm)]
fn type_of_name(ctx: &ThreadContext, object: Word) -> Result<&'static str, ObjectError> {
    Ok(match classify_object(ctx, object) {
        ObjectRef::Fixnum(_) => "FIXNUM",
        ObjectRef::Character(_) => "CHARACTER",
        ObjectRef::Cons(_) => "CONS",
        ObjectRef::Symbol(_) => {
            if object == Word::NIL {
                "NULL"
            } else if crate::typep::is_keyword(ctx, object).map_err(|_| ObjectError::TypeError)? {
                "KEYWORD"
            } else {
                "SYMBOL"
            }
        }
        ObjectRef::HashTable(_) => "HASH-TABLE",
        ObjectRef::String(_) => "SIMPLE-STRING",
        ObjectRef::SimpleVector(_) => "SIMPLE-VECTOR",
        ObjectRef::SpecializedArray(_) => {
            if ncl_object::specialized_array_element_type(ctx, object)?
                == ncl_object::ArrayElementType::Bit
            {
                "SIMPLE-BIT-VECTOR"
            } else {
                "SIMPLE-ARRAY"
            }
        }
        ObjectRef::Array(_) => "ARRAY",
        ObjectRef::Function(_) => "COMPILED-FUNCTION",
        ObjectRef::Closure(_) => "FUNCTION",
        ObjectRef::Instance(_) => "STANDARD-OBJECT",
        ObjectRef::Structure(_) => "STRUCTURE-OBJECT",
        ObjectRef::Bignum(_) => "BIGNUM",
        ObjectRef::Ratio(_) => "RATIO",
        ObjectRef::DoubleFloat(_) => "DOUBLE-FLOAT",
        ObjectRef::Complex(_) => "COMPLEX",
        ObjectRef::Package(_) => "PACKAGE",
        ObjectRef::Readtable(_) => "READTABLE",
        ObjectRef::Stream(_) => "STREAM",
        _ => "T", // check-added-lines: allow(wildcard) ObjectRef is non_exhaustive; residual kinds are simply of type T.
    })
}

fn type_of_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let object = args.required(0)?;
    let name = type_of_name(ctx, object)?;
    intern_common_lisp(ctx, runtime, name)
}

fn subtypep_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let type1 = args.required(0)?;
    let type2 = args.required(1)?;
    let spec1 = parse_type_specifier(ctx, type1);
    let spec2 = parse_type_specifier(ctx, type2);
    // Every failure mode (an unresolved `TypeError` from `subtypep`, or a
    // specifier that fails to parse at all) is treated as "cannot determine"
    // rather than a hard error, matching CLHS's permission for SUBTYPEP to
    // report uncertainty via its second value.
    let (is_sub, definite) = match (spec1, spec2) {
        (Ok(spec1), Ok(spec2)) => subtypep(&spec1, &spec2).unwrap_or_default(),
        (Err(_), _) | (_, Err(_)) => (false, false),
    };
    values.set(&[bool_word(definite)]);
    Ok(bool_word(is_sub))
}

fn coerce_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let object = args.required(0)?;
    let type_word = args.required(1)?;
    let spec = parse_type_specifier(ctx, type_word).map_err(|_| ObjectError::TypeError)?;
    if typep(ctx, object, &spec).map_err(|_| ObjectError::TypeError)? {
        return Ok(object);
    }
    coerce_to(ctx, runtime, object, &spec)
}

fn coerce_to(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
    spec: &TypeSpecifier,
) -> Result<Word, ObjectError> {
    match spec {
        TypeSpecifier::Named(named) => coerce_to_named(ctx, runtime, object, *named),
        TypeSpecifier::Vector { .. } => coerce_to_vector(ctx, runtime, object),
        TypeSpecifier::Array { .. }
        | TypeSpecifier::IntegerRange { .. }
        | TypeSpecifier::Or(_)
        | TypeSpecifier::And(_)
        | TypeSpecifier::Not(_)
        | TypeSpecifier::Member(_)
        | TypeSpecifier::Eql(_)
        | TypeSpecifier::Satisfies(_)
        | TypeSpecifier::Cons { .. }
        | TypeSpecifier::Function { .. }
        | TypeSpecifier::Values(_)
        | TypeSpecifier::Deftype { .. } => coerce_to_named_fallback(ctx, object, spec),
    }
}

/// `(function ...)` designators coerce independently of the array/list/float
/// machinery below; everything else in this catch-all has no coercion rule
/// defined for this phase.
fn coerce_to_named_fallback(
    ctx: &ThreadContext,
    object: Word,
    spec: &TypeSpecifier,
) -> Result<Word, ObjectError> {
    if matches!(spec, TypeSpecifier::Function { .. }) {
        return coerce_to_function(ctx, object);
    }
    Err(ObjectError::TypeError)
}

#[allow(clippy::too_many_lines)]
fn coerce_to_named(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
    named: NamedType,
) -> Result<Word, ObjectError> {
    match named {
        NamedType::List => coerce_to_list(ctx, runtime, object),
        NamedType::Vector | NamedType::SimpleVector | NamedType::Array | NamedType::SimpleArray => {
            coerce_to_vector(ctx, runtime, object)
        }
        NamedType::String
        | NamedType::SimpleString
        | NamedType::BaseString
        | NamedType::SimpleBaseString => coerce_to_string(ctx, runtime, object),
        NamedType::Character
        | NamedType::BaseChar
        | NamedType::StandardChar
        | NamedType::ExtendedChar => coerce_to_character(ctx, object),
        NamedType::Float
        | NamedType::ShortFloat
        | NamedType::SingleFloat
        | NamedType::DoubleFloat
        | NamedType::LongFloat => coerce_to_float(ctx, runtime, object),
        NamedType::Function | NamedType::CompiledFunction => coerce_to_function(ctx, object),
        NamedType::T
        | NamedType::Nil
        | NamedType::Boolean
        | NamedType::Symbol
        | NamedType::Keyword
        | NamedType::Package
        | NamedType::Cons
        | NamedType::Null
        | NamedType::Atom
        | NamedType::Number
        | NamedType::Real
        | NamedType::Rational
        | NamedType::Integer
        | NamedType::Fixnum
        | NamedType::Bignum
        | NamedType::Ratio
        | NamedType::Complex
        | NamedType::BitVector
        | NamedType::SimpleBitVector
        | NamedType::Sequence
        | NamedType::HashTable
        | NamedType::Stream
        | NamedType::RandomState
        | NamedType::Restart
        | NamedType::Structure
        | NamedType::ValuesType => Err(ObjectError::TypeError),
    }
}

#[allow(clippy::wildcard_enum_match_arm)]
fn sequence_elements(ctx: &ThreadContext, object: Word) -> Result<Vec<Word>, ObjectError> {
    match classify_object(ctx, object) {
        ObjectRef::SimpleVector(_) => (0..simple_vector_length(ctx, object)?)
            .map(|index| simple_vector_ref(ctx, object, index))
            .collect(),
        ObjectRef::String(_) => (0..string_length(ctx, object)?)
            .map(|index| string_ref(ctx, object, index).map(|c| Word::character(u32::from(c))))
            .collect(),
        ObjectRef::Cons(_) => list_elements(ctx, object),
        _ if object == Word::NIL => Ok(Vec::new()),
        _ => Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) only sequences have elements.
    }
}

fn list_elements(ctx: &ThreadContext, mut cursor: Word) -> Result<Vec<Word>, ObjectError> {
    let mut result = Vec::new();
    while cursor != Word::NIL {
        if !cursor.is_cons() {
            return Err(ObjectError::TypeError);
        }
        result.push(car(ctx, cursor)?);
        cursor = cdr(ctx, cursor)?;
    }
    Ok(result)
}

fn list_from(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let locals = values
        .iter()
        .copied()
        .map(Local::from_word)
        .collect::<Vec<_>>();
    let handles = scope.root_many(&locals);
    let mut result = scope.root(Local::from_word(Word::NIL));
    for value in handles.iter().rev() {
        result = scope.make_cons(runtime, *value, result)?;
    }
    Ok(scope.get(result).as_word())
}

fn coerce_to_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
) -> Result<Word, ObjectError> {
    let elements = sequence_elements(ctx, object)?;
    list_from(ctx, runtime, &elements)
}

fn coerce_to_vector(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
) -> Result<Word, ObjectError> {
    let elements = sequence_elements(ctx, object)?;
    make_simple_vector(ctx, runtime, &elements)
}

fn coerce_to_string(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
) -> Result<Word, ObjectError> {
    let elements = sequence_elements(ctx, object)?;
    let chars = elements
        .iter()
        .map(|word| {
            word.as_character()
                .and_then(char::from_u32)
                .ok_or(ObjectError::TypeError)
        })
        .collect::<Result<Vec<_>, _>>()?;
    make_string(ctx, runtime, &chars)
}

#[allow(clippy::wildcard_enum_match_arm)]
fn coerce_to_character(ctx: &ThreadContext, object: Word) -> Result<Word, ObjectError> {
    match classify_object(ctx, object) {
        ObjectRef::String(_) if string_length(ctx, object)? == 1 => {
            string_ref(ctx, object, 0).map(|c| Word::character(u32::from(c)))
        }
        _ => Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) only a length-1 string converts.
    }
}

fn fixnum_to_f64(value: i64) -> f64 {
    let magnitude = value.unsigned_abs();
    let low = u32::try_from(magnitude & u64::from(u32::MAX)).unwrap_or(0);
    let high = u32::try_from(magnitude >> 32).unwrap_or(0);
    let result = f64::from(high).mul_add(2_f64.powi(32), f64::from(low));
    if value.is_negative() { -result } else { result }
}

fn bignum_to_f64(ctx: &ThreadContext, object: Bignum) -> Result<f64, ObjectError> {
    let sign = bignum_sign(ctx, object)?;
    let magnitude = bignum_limbs(ctx, object)?
        .iter()
        .rev()
        .fold(0.0_f64, |accumulator, limb| {
            accumulator.mul_add(f64::from(u32::MAX) + 1.0, f64::from(*limb))
        });
    Ok(if sign { -magnitude } else { magnitude })
}

#[allow(clippy::wildcard_enum_match_arm)]
fn real_to_f64(ctx: &ThreadContext, word: Word) -> Result<f64, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(value) => Ok(fixnum_to_f64(value)),
        ObjectRef::DoubleFloat(_) => double_value(ctx, DoubleFloat::from_word(word)),
        ObjectRef::Bignum(_) => bignum_to_f64(ctx, Bignum::from_word(word)),
        ObjectRef::Ratio(_) => {
            let numerator = real_to_f64(ctx, ratio_numerator(ctx, Ratio::from_word(word))?)?;
            let denominator = real_to_f64(ctx, ratio_denominator(ctx, Ratio::from_word(word))?)?;
            Ok(numerator / denominator)
        }
        _ => Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) only real numbers convert to float.
    }
}

fn coerce_to_float(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
) -> Result<Word, ObjectError> {
    let value = real_to_f64(ctx, object)?;
    make_double(ctx, runtime, value).map(Into::into)
}

#[allow(clippy::wildcard_enum_match_arm)]
fn coerce_to_function(ctx: &ThreadContext, object: Word) -> Result<Word, ObjectError> {
    match classify_object(ctx, object) {
        ObjectRef::Function(_) | ObjectRef::Closure(_) => Ok(object),
        ObjectRef::Symbol(_) if object != Word::NIL => {
            let function = symbol_function(ctx, object)?;
            if function == Word::UNBOUND {
                // check-added-lines: allow(unbound) sentinel check
                Err(ObjectError::TypeError)
            } else {
                Ok(function)
            }
        }
        _ => Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) only function designators convert.
    }
}

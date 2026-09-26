//! Random-state objects and Common Lisp numeric constants.

use ncl_object::{
    classify_object, instance_class, make_double, make_instance, set_symbol_constant,
    set_symbol_special, set_symbol_value, slot_ref, slot_set, Builtin, BuiltinArgs,
    BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName, BuiltinPackage,
    LambdaList, MultipleValues, ObjectError, ObjectRef, Package, Parameter, ParameterType, Runtime,
    ThreadContext, Word,
};

use crate::MOST_POSITIVE_FIXNUM;

fn u64_to_f64(value: u64) -> Result<f64, ObjectError> {
    let high = u32::try_from(value >> 32).map_err(|_| ObjectError::Layout)?;
    let low = u32::try_from(value & u64::from(u32::MAX)).map_err(|_| ObjectError::Layout)?;
    Ok(f64::from(high) * 4_294_967_296.0 + f64::from(low))
}

const STATE_SLOT: usize = 0;
const INITIAL_SEED: i64 = 0x0135_79bd_f246_8acd;
const ANY: Parameter = Parameter {
    name: BuiltinName::new("OBJECT"),
    ty: ParameterType::Any,
};
const RANDOM_REQUIRED: &[Parameter] = &[Parameter {
    name: BuiltinName::new("LIMIT"),
    ty: ParameterType::Number,
}];

fn random_state_class(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Word, ObjectError> {
    if let Some(class) = runtime.class(ctx, "RANDOM-STATE") {
        return Ok(class);
    }
    runtime.define_class(ctx, "RANDOM-STATE", Word::fixnum(1))?;
    runtime
        .class(ctx, "RANDOM-STATE")
        .ok_or(ObjectError::Layout)
}

fn state_p(ctx: &mut ThreadContext, runtime: &Runtime, value: Word) -> Result<bool, ObjectError> {
    let ObjectRef::Instance(_) = classify_object(ctx, value) else {
        return Ok(false);
    };
    Ok(instance_class(ctx, ncl_object::Instance::from_word(value))?
        == runtime
            .class(ctx, "RANDOM-STATE")
            .ok_or(ObjectError::Layout)?)
}

fn make_state(ctx: &mut ThreadContext, runtime: &Runtime, seed: i64) -> Result<Word, ObjectError> {
    let class = random_state_class(ctx, runtime)?;
    Ok(make_instance(
        ctx,
        runtime,
        class,
        &[Word::fixnum(seed & MOST_POSITIVE_FIXNUM)],
    )?
    .into())
}

fn next_word(ctx: &mut ThreadContext, state: Word) -> Result<u64, ObjectError> {
    let instance = ncl_object::Instance::from_word(state);
    let mut seed = slot_ref(ctx, instance, STATE_SLOT)?
        .as_fixnum()
        .ok_or(ObjectError::TypeError)
        .and_then(|seed| u64::try_from(seed).map_err(|_| ObjectError::TypeError))?;
    seed ^= seed << 13;
    seed ^= seed >> 17;
    seed ^= seed << 5;
    let mask = u64::try_from(MOST_POSITIVE_FIXNUM).map_err(|_| ObjectError::TypeError)?;
    let next = i64::try_from(seed & mask).map_err(|_| ObjectError::TypeError)?;
    slot_set(ctx, instance, STATE_SLOT, Word::fixnum(next))?;
    u64::try_from(next).map_err(|_| ObjectError::TypeError)
}

fn random_u128(ctx: &mut ThreadContext, state: Word, bits: u32) -> Result<u128, ObjectError> {
    if bits == 0 {
        return Ok(0);
    }
    if bits > 127 {
        return Err(ObjectError::TypeError);
    }
    let first = u128::from(next_word(ctx, state)?);
    let second = u128::from(next_word(ctx, state)?);
    let third = u128::from(next_word(ctx, state)?);
    let value = first
        | second.checked_shl(62).ok_or(ObjectError::TypeError)?
        | third.checked_shl(124).ok_or(ObjectError::TypeError)?;
    let mask = 1_u128
        .checked_shl(bits)
        .and_then(|value| value.checked_sub(1))
        .ok_or(ObjectError::TypeError)?;
    Ok(value & mask)
}

fn random_below(ctx: &mut ThreadContext, state: Word, bound: i128) -> Result<i128, ObjectError> {
    let bound = u128::try_from(bound).map_err(|_| ObjectError::TypeError)?;
    let upper = bound.checked_sub(1).ok_or(ObjectError::TypeError)?;
    let bits = u128::BITS - upper.leading_zeros();
    loop {
        let value = random_u128(ctx, state, bits)?;
        if value < bound {
            return i128::try_from(value).map_err(|_| ObjectError::TypeError);
        }
    }
}

fn random_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let limit = args.required(0)?;
    let state = args.get(1).map_or_else(
        || -> Result<Word, ObjectError> {
            let package = runtime
                .find_package(ctx, "COMMON-LISP")
                .ok_or(ObjectError::PackageConflict)?;
            let (symbol, _) = Package::from_word(package).intern(ctx, runtime, "*RANDOM-STATE*")?;
            ncl_object::symbol_value(ctx, symbol)
        },
        Ok,
    )?;
    if !state_p(ctx, runtime, state)? {
        return Err(ObjectError::TypeError);
    }
    match classify_object(ctx, limit) {
        ObjectRef::DoubleFloat(value) => {
            let bound = ncl_object::double_value(ctx, ncl_object::DoubleFloat::from_word(value))?;
            if !bound.is_finite() || bound <= 0.0 {
                return Err(ObjectError::TypeError);
            }
            let modulus = u64::try_from(MOST_POSITIVE_FIXNUM)
                .ok()
                .and_then(|value| value.checked_add(1))
                .ok_or(ObjectError::TypeError)?;
            let fraction = u64_to_f64(next_word(ctx, state)?)? / u64_to_f64(modulus)?;
            make_double(ctx, runtime, bound * fraction).map(Into::into)
        }
        ObjectRef::Fixnum(_) | ObjectRef::Bignum(_) => {
            let bound = crate::bitops::integer(ctx, limit)?;
            if bound <= 0 {
                return Err(ObjectError::TypeError);
            }
            let value = random_below(ctx, state, bound)?;
            i64::try_from(value)
                .ok()
                .and_then(|value| {
                    let word = Word::fixnum(value);
                    (word.as_fixnum() == Some(value)).then_some(word)
                })
                .map_or_else(
                    || ncl_object::make_bignum_from_i128(ctx, runtime, value).map(Into::into),
                    Ok,
                )
        }
        _ => Err(ObjectError::TypeError),
    }
}

fn make_random_state_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    match args.get(0) {
        None | Some(Word::NIL) => make_state(ctx, runtime, INITIAL_SEED),
        Some(Word::TRUE) => make_state(ctx, runtime, INITIAL_SEED ^ 0x5dee_ce66),
        Some(value) if state_p(ctx, runtime, value)? => {
            let seed = slot_ref(ctx, ncl_object::Instance::from_word(value), STATE_SLOT)?
                .as_fixnum()
                .ok_or(ObjectError::TypeError)?;
            make_state(ctx, runtime, seed)
        }
        Some(_) => Err(ObjectError::TypeError),
    }
}

fn random_state_p_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(if state_p(ctx, runtime, args.required(0)?)? {
        Word::TRUE
    } else {
        Word::NIL
    })
}

fn set_constant(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    package: Word,
    name: &'static str,
    value: Word,
) -> Result<(), ObjectError> {
    let (symbol, _) = Package::from_word(package).intern(ctx, runtime, name)?;
    set_symbol_constant(ctx, symbol, true)?;
    set_symbol_value(ctx, symbol, value)
}

fn set_float_constant(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    package: Word,
    name: &'static str,
    value: f64,
) -> Result<(), ObjectError> {
    let value = make_double(ctx, runtime, value)?.into();
    set_constant(ctx, runtime, package, name, value)
}

fn register_constants(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    let package = runtime
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    set_constant(
        ctx,
        runtime,
        package,
        "MOST-POSITIVE-FIXNUM",
        Word::fixnum(crate::MOST_POSITIVE_FIXNUM),
    )?;
    set_constant(
        ctx,
        runtime,
        package,
        "MOST-NEGATIVE-FIXNUM",
        Word::fixnum(crate::MOST_NEGATIVE_FIXNUM),
    )?;
    set_float_constant(ctx, runtime, package, "PI", std::f64::consts::PI)?;
    for name in [
        "SHORT-FLOAT-EPSILON",
        "SINGLE-FLOAT-EPSILON",
        "DOUBLE-FLOAT-EPSILON",
        "LONG-FLOAT-EPSILON",
    ] {
        set_float_constant(ctx, runtime, package, name, f64::EPSILON)?;
    }
    for name in [
        "SHORT-FLOAT-NEGATIVE-EPSILON",
        "SINGLE-FLOAT-NEGATIVE-EPSILON",
        "DOUBLE-FLOAT-NEGATIVE-EPSILON",
        "LONG-FLOAT-NEGATIVE-EPSILON",
    ] {
        set_float_constant(ctx, runtime, package, name, f64::EPSILON / 2.0)?;
    }
    for name in [
        "LEAST-POSITIVE-SHORT-FLOAT",
        "LEAST-POSITIVE-SINGLE-FLOAT",
        "LEAST-POSITIVE-DOUBLE-FLOAT",
        "LEAST-POSITIVE-LONG-FLOAT",
        "LEAST-POSITIVE-NORMALIZED-SHORT-FLOAT",
        "LEAST-POSITIVE-NORMALIZED-SINGLE-FLOAT",
        "LEAST-POSITIVE-NORMALIZED-DOUBLE-FLOAT",
        "LEAST-POSITIVE-NORMALIZED-LONG-FLOAT",
    ] {
        set_float_constant(ctx, runtime, package, name, f64::MIN_POSITIVE)?;
    }
    for name in [
        "LEAST-NEGATIVE-SHORT-FLOAT",
        "LEAST-NEGATIVE-SINGLE-FLOAT",
        "LEAST-NEGATIVE-DOUBLE-FLOAT",
        "LEAST-NEGATIVE-LONG-FLOAT",
        "LEAST-NEGATIVE-NORMALIZED-SHORT-FLOAT",
        "LEAST-NEGATIVE-NORMALIZED-SINGLE-FLOAT",
        "LEAST-NEGATIVE-NORMALIZED-DOUBLE-FLOAT",
        "LEAST-NEGATIVE-NORMALIZED-LONG-FLOAT",
    ] {
        set_float_constant(ctx, runtime, package, name, -f64::MIN_POSITIVE)?;
    }
    for name in [
        "MOST-POSITIVE-SHORT-FLOAT",
        "MOST-POSITIVE-SINGLE-FLOAT",
        "MOST-POSITIVE-DOUBLE-FLOAT",
        "MOST-POSITIVE-LONG-FLOAT",
    ] {
        set_float_constant(ctx, runtime, package, name, f64::MAX)?;
    }
    for name in [
        "MOST-NEGATIVE-SHORT-FLOAT",
        "MOST-NEGATIVE-SINGLE-FLOAT",
        "MOST-NEGATIVE-DOUBLE-FLOAT",
        "MOST-NEGATIVE-LONG-FLOAT",
    ] {
        set_float_constant(ctx, runtime, package, name, -f64::MAX)?;
    }
    Ok(())
}

/// Register random-state support and numeric constants.
pub fn register(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    register_constants(ctx, runtime)?;
    let package = runtime
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    let (symbol, _) = Package::from_word(package).intern(ctx, runtime, "*RANDOM-STATE*")?;
    set_symbol_special(ctx, symbol, true)?;
    let state = make_state(ctx, runtime, INITIAL_SEED)?;
    set_symbol_value(ctx, symbol, state)?;

    let descriptor = Builtin {
        lambda_list: LambdaList::with_optional(RANDOM_REQUIRED, &[ANY]),
        convention: BuiltinConvention::Adapted,
    };
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("RANDOM")),
        BuiltinImplementation::direct(descriptor, random_builtin),
    )?;
    let descriptor = Builtin {
        lambda_list: LambdaList::with_optional(&[], &[ANY]),
        convention: BuiltinConvention::Adapted,
    };
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::CommonLisp,
            BuiltinName::new("MAKE-RANDOM-STATE"),
        ),
        BuiltinImplementation::direct(descriptor, make_random_state_builtin),
    )?;
    let descriptor = Builtin {
        lambda_list: LambdaList::fixed(&[ANY]),
        convention: BuiltinConvention::Direct(ncl_object::Arity::exact(1)),
    };
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::CommonLisp,
            BuiltinName::new("RANDOM-STATE-P"),
        ),
        BuiltinImplementation::direct(descriptor, random_state_p_builtin),
    )?;
    Ok(())
}

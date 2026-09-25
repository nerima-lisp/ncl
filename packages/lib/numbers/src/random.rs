//! Random-state objects and the Common Lisp random builtins.

use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, LambdaList, MultipleValues, ObjectError, ObjectRef, Package, Parameter,
    ParameterType, Runtime, ThreadContext, Word, classify_object, instance_class, make_double,
    make_instance, set_symbol_special, set_symbol_value, slot_ref, slot_set,
};

const STATE_SLOT: usize = 0;
const INITIAL_SEED: i64 = 0x1357_9bdf_2468_acd;
const ANY: Parameter = Parameter {
    name: BuiltinName::new("OBJECT"),
    ty: ParameterType::Any,
};

fn random_state_class(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Word, ObjectError> {
    if let Some(class) = runtime.class(ctx, "RANDOM-STATE") {
        return Ok(class);
    }
    runtime.define_class(ctx, "RANDOM-STATE", Word::fixnum(1))?;
    runtime.class(ctx, "RANDOM-STATE").ok_or(ObjectError::Layout)
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
        &[Word::fixnum(seed & ((1_i64 << 62) - 1))],
    )?
    .into())
}

fn next_word(ctx: &mut ThreadContext, state: Word) -> Result<u64, ObjectError> {
    let instance = ncl_object::Instance::from_word(state);
    let mut seed = slot_ref(ctx, instance, STATE_SLOT)?
        .as_fixnum()
        .ok_or(ObjectError::TypeError)? as u64;
    seed ^= seed << 13;
    seed ^= seed >> 17;
    seed ^= seed << 5;
    let next = (seed & ((1_u64 << 62) - 1)) as i64;
    slot_set(ctx, instance, STATE_SLOT, Word::fixnum(next))?;
    Ok(seed)
}

fn integer(ctx: &ThreadContext, value: Word) -> Result<i128, ObjectError> {
    match classify_object(ctx, value) {
        ObjectRef::Fixnum(value) => Ok(i128::from(value)),
        ObjectRef::Bignum(value) => {
            let bignum = ncl_object::Bignum::from_word(value);
            let magnitude = ncl_object::bignum_limbs(ctx, bignum)?
                .into_iter()
                .enumerate()
                .try_fold(
                    0_i128,
                    |value, (index, limb)| -> Result<i128, ObjectError> {
                        let shift = index.checked_mul(32).ok_or(ObjectError::Layout)?;
                        Ok(value | (i128::from(limb) << shift))
                    },
                )?;
            Ok(if ncl_object::bignum_sign(ctx, bignum)? {
                -magnitude
            } else {
                magnitude
            })
        }
        _ => Err(ObjectError::TypeError),
    }
}

fn random_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let limit = args.required(0)?;
    let state = if let Some(state) = args.get(1) {
        state
    } else {
        let package = runtime
            .find_package(ctx, "COMMON-LISP")
            .ok_or(ObjectError::PackageConflict)?;
        let (symbol, _) = Package::from_word(package).intern(ctx, runtime, "*RANDOM-STATE*")?;
        ncl_object::symbol_value(ctx, symbol)?
    };
    if !state_p(ctx, runtime, state)? {
        return Err(ObjectError::TypeError);
    }
    match classify_object(ctx, limit) {
        ObjectRef::DoubleFloat(value) => {
            let bound = ncl_object::double_value(ctx, ncl_object::DoubleFloat::from_word(value))?;
            if !bound.is_finite() || bound <= 0.0 {
                return Err(ObjectError::TypeError);
            }
            let fraction = (next_word(ctx, state)? as f64) / ((1_u64 << 62) as f64);
            make_double(ctx, runtime, bound * fraction).map(Into::into)
        }
        ObjectRef::Fixnum(_) | ObjectRef::Bignum(_) => {
            let bound = integer(ctx, limit)?;
            if bound <= 0 {
                return Err(ObjectError::TypeError);
            }
            let value = i128::from(next_word(ctx, state)?) % bound;
            if let Ok(value) = i64::try_from(value) {
                Ok(Word::fixnum(value))
            } else {
                ncl_object::make_bignum_from_i128(ctx, runtime, value).map(Into::into)
            }
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
        Some(Word::TRUE) => make_state(ctx, runtime, INITIAL_SEED ^ 0x5deece66d),
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

/// Register random-state support and the random builtins.
pub fn register(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    let package = runtime
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    let (symbol, _) = Package::from_word(package).intern(ctx, runtime, "*RANDOM-STATE*")?;
    set_symbol_special(ctx, symbol, true)?;
    let state = make_state(ctx, runtime, INITIAL_SEED)?;
    set_symbol_value(ctx, symbol, state)?;

    let optional = &[ANY];
    const RANDOM_REQUIRED: &[Parameter] = &[Parameter {
        name: BuiltinName::new("LIMIT"),
        ty: ParameterType::Number,
    }];
    for (name, required, callback) in [
        (
            "RANDOM",
            RANDOM_REQUIRED,
            random_builtin as ncl_object::RustBuiltin,
        ),
        (
            "MAKE-RANDOM-STATE",
            &[][..],
            make_random_state_builtin as ncl_object::RustBuiltin,
        ),
    ] {
        let descriptor = Builtin {
            lambda_list: LambdaList::with_optional(required, optional),
            convention: BuiltinConvention::Adapted,
        };
        runtime.register_builtin(
            ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            BuiltinImplementation::direct(descriptor, callback),
        )?;
    }
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

//! ANSI Common Lisp numeric builtins.

mod arithmetic;
mod bitops;
mod complex;
mod constants;
mod random;
mod rational_float;
mod remainder;
mod rounding;
mod transcendental;

use core::cell::Cell;
use ncl_object::{
    Arity, Builtin, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, LambdaList, ObjectError, Package, Parameter, ParameterType, Runtime,
    RustBuiltin, ThreadContext, Word, set_symbol_constant, set_symbol_special, set_symbol_value,
};
use ncl_sys::RootSlot;

fn with_root<T>(
    ctx: &mut ThreadContext,
    value: &mut Word,
    f: impl FnOnce(&mut ThreadContext, RootSlot<'_>) -> Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    let mut slot = Cell::new(*value);
    let token = ncl_object::push_root(ctx, slot.get_mut());
    let result = f(ctx, RootSlot::new(&slot));
    *value = slot.get();
    if !ncl_object::pop_root(ctx, token) {
        return Err(ObjectError::Layout);
    }
    result
}

#[derive(Clone, Copy)]
enum NativeEntry {
    Add,
    Mul,
}

impl NativeEntry {
    fn address(self) -> Result<usize, ObjectError> {
        let address = match self {
            Self::Add => ncl_sys::function_address!(ncl_sys::native_add),
            Self::Mul => ncl_sys::function_address!(ncl_sys::native_mul),
        }
        .map_err(|_| ObjectError::Layout)?;
        usize::try_from(address).map_err(|_| ObjectError::Layout)
    }
}

pub(crate) const MOST_POSITIVE_FIXNUM: i64 = i64::MAX >> ncl_sys::FIXNUM_TAG_BITS;
pub(crate) const MOST_NEGATIVE_FIXNUM: i64 = i64::MIN >> ncl_sys::FIXNUM_TAG_BITS;

fn install(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &'static str,
    arity: u8,
    direct: bool,
    callback: RustBuiltin,
    native_entry: Option<NativeEntry>,
) -> Result<(), ObjectError> {
    const NO_PARAMETERS: &[Parameter] = &[];
    const ONE_PARAMETER: &[Parameter] = &[Parameter {
        name: BuiltinName::new("NUMBER"),
        ty: ParameterType::Number,
    }];
    const TWO_PARAMETERS: &[Parameter] = &[
        Parameter {
            name: BuiltinName::new("NUMBER"),
            ty: ParameterType::Number,
        },
        Parameter {
            name: BuiltinName::new("DIVISOR"),
            ty: ParameterType::Number,
        },
    ];
    const THREE_PARAMETERS: &[Parameter] = &[
        Parameter {
            name: BuiltinName::new("NUMBER"),
            ty: ParameterType::Number,
        },
        Parameter {
            name: BuiltinName::new("NUMBER"),
            ty: ParameterType::Number,
        },
        Parameter {
            name: BuiltinName::new("NUMBER"),
            ty: ParameterType::Number,
        },
    ];
    let descriptor = if direct {
        Builtin {
            lambda_list: LambdaList::fixed(match arity {
                0 => NO_PARAMETERS,
                1 => ONE_PARAMETER,
                2 => TWO_PARAMETERS,
                3 => THREE_PARAMETERS,
                _ => return Err(ObjectError::TypeError),
            }),
            convention: BuiltinConvention::Direct(Arity::exact(arity)),
        }
    } else {
        Builtin {
            lambda_list: LambdaList::with_rest(
                &[],
                Parameter {
                    name: BuiltinName::new("NUMBER"),
                    ty: ParameterType::Number,
                },
            ),
            convention: BuiltinConvention::Adapted,
        }
    };
    let identifier = BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name));
    let implementation = BuiltinImplementation::direct(descriptor, callback);
    let implementation = native_entry
        .map(NativeEntry::address)
        .transpose()?
        .map_or(implementation, |entry| implementation.with_entry(entry));
    runtime
        .register_builtin(ctx, identifier, implementation)
        .map(|_| ())
}

fn install_set(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    entries: &[(&'static str, u8, bool, RustBuiltin, Option<NativeEntry>)],
) -> Result<(), ObjectError> {
    for &(name, arity, direct, callback, native_entry) in entries {
        install(runtime, ctx, name, arity, direct, callback, native_entry)?;
    }
    Ok(())
}

fn install_optional_set(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    entries: &[(&'static str, RustBuiltin)],
) -> Result<(), ObjectError> {
    const REQUIRED: &[Parameter] = &[Parameter {
        name: BuiltinName::new("NUMBER"),
        ty: ParameterType::Number,
    }];
    const OPTIONAL: &[Parameter] = &[Parameter {
        name: BuiltinName::new("DIVISOR"),
        ty: ParameterType::Number,
    }];
    for &(name, callback) in entries {
        let descriptor = Builtin {
            lambda_list: LambdaList::with_optional(REQUIRED, OPTIONAL),
            convention: BuiltinConvention::Adapted,
        };
        runtime.register_builtin(
            ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            BuiltinImplementation::direct(descriptor, callback),
        )?;
    }
    Ok(())
}

fn install_rational_float(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    const ONE: &[Parameter] = &[Parameter {
        name: BuiltinName::new("NUMBER"),
        ty: ParameterType::Number,
    }];
    const TWO_FLOATS: &[Parameter] = &[
        Parameter {
            name: BuiltinName::new("FLOAT"),
            ty: ParameterType::Number,
        },
        Parameter {
            name: BuiltinName::new("SCALE"),
            ty: ParameterType::Number,
        },
    ];
    for (name, callback) in [
        ("NUMERATOR", rational_float::numerator as RustBuiltin),
        ("DENOMINATOR", rational_float::denominator as RustBuiltin),
        ("RATIONAL", rational_float::rational as RustBuiltin),
        ("FLOAT", rational_float::float as RustBuiltin),
        ("DECODE-FLOAT", rational_float::decode_float as RustBuiltin),
        (
            "INTEGER-DECODE-FLOAT",
            rational_float::integer_decode_float as RustBuiltin,
        ),
        ("FLOAT-DIGITS", rational_float::float_digits as RustBuiltin),
        (
            "FLOAT-PRECISION",
            rational_float::float_precision as RustBuiltin,
        ),
        ("FLOAT-RADIX", rational_float::float_radix as RustBuiltin),
    ] {
        let descriptor = Builtin {
            lambda_list: LambdaList::fixed(ONE),
            convention: BuiltinConvention::Direct(Arity::exact(1)),
        };
        runtime.register_builtin(
            ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            BuiltinImplementation::direct(descriptor, callback),
        )?;
    }
    let scale = Builtin {
        lambda_list: LambdaList::fixed(TWO_FLOATS),
        convention: BuiltinConvention::Direct(Arity::exact(2)),
    };
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("SCALE-FLOAT")),
        BuiltinImplementation::direct(scale, rational_float::scale_float),
    )?;
    install_optional_set(
        runtime,
        ctx,
        &[
            ("RATIONALIZE", rational_float::rationalize),
            ("FLOAT-SIGN", rational_float::float_sign),
        ],
    )
}

/// Register numeric predicates, arithmetic, rounding, and integer operations.
///
/// # Errors
///
/// Returns an error if registration or package initialization fails.
#[allow(clippy::too_many_lines)]
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    install_set(
        runtime,
        &mut ctx,
        &[
            ("NUMBERP", 1, true, arithmetic::typed_numberp, None),
            ("INTEGERP", 1, true, arithmetic::typed_integerp, None),
            ("RATIONALP", 1, true, arithmetic::typed_rationalp, None),
            ("FLOATP", 1, true, arithmetic::typed_floatp, None),
            ("REALP", 1, true, arithmetic::typed_realp, None),
            ("COMPLEXP", 1, true, arithmetic::typed_complexp, None),
            ("+", 0, false, arithmetic::typed_add, Some(NativeEntry::Add)),
            ("-", 0, false, arithmetic::typed_sub, None),
            ("*", 0, false, arithmetic::typed_mul, Some(NativeEntry::Mul)),
            ("/", 0, false, arithmetic::typed_div, None),
            ("=", 0, false, arithmetic::typed_equal, None),
            ("EQ", 2, true, arithmetic::typed_eq, None),
            ("EQL", 2, true, arithmetic::typed_eql, None),
            ("/=", 0, false, arithmetic::typed_not_equal, None),
            ("<", 0, false, arithmetic::typed_less, None),
            (">", 0, false, arithmetic::typed_greater, None),
            ("<=", 0, false, arithmetic::typed_less_equal, None),
            (">=", 0, false, arithmetic::typed_greater_equal, None),
            ("MAX", 0, false, arithmetic::typed_max, None),
            ("MIN", 0, false, arithmetic::typed_min, None),
            ("1+", 1, true, arithmetic::typed_one_plus, None),
            ("1-", 1, true, arithmetic::typed_one_minus, None),
            ("ABS", 1, true, arithmetic::typed_abs, None),
            ("SIGNUM", 1, true, arithmetic::typed_signum, None),
            ("ZEROP", 1, true, arithmetic::typed_zerop, None),
            ("PLUSP", 1, true, arithmetic::typed_plusp, None),
            ("MINUSP", 1, true, arithmetic::typed_minusp, None),
            ("EVENP", 1, true, arithmetic::typed_evenp, None),
            ("ODDP", 1, true, arithmetic::typed_oddp, None),
            ("MOD", 2, true, remainder::typed_mod, None),
            ("REM", 2, true, remainder::typed_rem, None),
            ("GCD", 0, false, remainder::typed_gcd, None),
            ("LCM", 0, false, remainder::typed_lcm, None),
            ("ISQRT", 1, true, remainder::typed_isqrt, None),
            ("EXP", 1, true, transcendental::typed_exp, None),
            ("EXPT", 2, true, transcendental::typed_expt, None),
            ("SQRT", 1, true, transcendental::typed_sqrt, None),
            ("SIN", 1, true, transcendental::typed_sin, None),
            ("COS", 1, true, transcendental::typed_cos, None),
            ("TAN", 1, true, transcendental::typed_tan, None),
            ("ASIN", 1, true, transcendental::typed_asin, None),
            ("ACOS", 1, true, transcendental::typed_acos, None),
            ("SINH", 1, true, transcendental::typed_sinh, None),
            ("COSH", 1, true, transcendental::typed_cosh, None),
            ("TANH", 1, true, transcendental::typed_tanh, None),
            ("ASINH", 1, true, transcendental::typed_asinh, None),
            ("ACOSH", 1, true, transcendental::typed_acosh, None),
            ("ATANH", 1, true, transcendental::typed_atanh, None),
            ("COMPLEX", 2, true, complex::typed_complex, None),
            ("CONJUGATE", 1, true, complex::typed_conjugate, None),
            ("CIS", 1, true, complex::typed_cis, None),
            ("PHASE", 1, true, complex::typed_phase, None),
            ("REALPART", 1, true, complex::typed_realpart, None),
            ("IMAGPART", 1, true, complex::typed_imagpart, None),
        ],
    )?;
    install_rational_float(runtime, &mut ctx)?;
    install_optional_set(
        runtime,
        &mut ctx,
        &[
            ("LOG", transcendental::typed_log),
            ("ATAN", transcendental::typed_atan),
        ],
    )?;
    install_optional_set(
        runtime,
        &mut ctx,
        &[
            ("FLOOR", rounding::typed_floor),
            ("CEILING", rounding::typed_ceiling),
            ("TRUNCATE", rounding::typed_truncate),
            ("ROUND", rounding::typed_round),
            ("FFLOOR", rounding::typed_ffloor),
            ("FCEILING", rounding::typed_fceiling),
            ("FTRUNCATE", rounding::typed_ftruncate),
            ("FROUND", rounding::typed_fround),
        ],
    )?;
    install_set(
        runtime,
        &mut ctx,
        &[
            ("LOGAND", 0, false, bitops::typed_logand, None),
            ("LOGIOR", 0, false, bitops::typed_logior, None),
            ("LOGXOR", 0, false, bitops::typed_logxor, None),
            ("LOGNOT", 1, true, bitops::typed_lognot, None),
            ("LOGEQV", 0, false, bitops::typed_logeqv, None),
            ("LOGNAND", 2, true, bitops::typed_lognand, None),
            ("LOGNOR", 2, true, bitops::typed_lognor, None),
            ("LOGANDC1", 2, true, bitops::typed_logandc1, None),
            ("LOGANDC2", 2, true, bitops::typed_logandc2, None),
            ("LOGORC1", 2, true, bitops::typed_logorc1, None),
            ("LOGORC2", 2, true, bitops::typed_logorc2, None),
            ("LOGTEST", 2, true, bitops::typed_logtest, None),
            ("LOGBITP", 2, true, bitops::typed_logbitp, None),
            ("LOGCOUNT", 1, true, bitops::typed_logcount, None),
            (
                "INTEGER-LENGTH",
                1,
                true,
                bitops::typed_integer_length,
                None,
            ),
            ("ASH", 2, true, bitops::typed_ash, None),
            ("BYTE", 2, true, bitops::typed_byte, None),
            ("BYTE-SIZE", 1, true, bitops::typed_byte_size, None),
            ("BYTE-POSITION", 1, true, bitops::typed_byte_position, None),
            ("LDB", 2, true, bitops::typed_ldb, None),
            ("DPB", 3, true, bitops::typed_dpb, None),
            ("LDB-TEST", 2, true, bitops::typed_ldb_test, None),
            ("MASK-FIELD", 2, true, bitops::typed_mask_field, None),
            ("DEPOSIT-FIELD", 3, true, bitops::typed_deposit_field, None),
            ("BOOLE", 3, true, bitops::typed_boole, None),
        ],
    )?;
    constants::register(&mut ctx, runtime)?;
    random::register(&mut ctx, runtime)?;
    let mut package = runtime
        .find_package(&ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    with_root(&mut ctx, &mut package, |ctx, package| {
        for name in ["*", "+", "-", "/"] {
            let (symbol, _) = Package::from_word(*package).intern(ctx, runtime, name)?;
            set_symbol_special(ctx, symbol, true)?;
            set_symbol_value(ctx, symbol, Word::NIL)?;
        }
        for (name, value) in [
            ("BOOLE-CLR", bitops::BOOLE_CLR),
            ("BOOLE-1", bitops::BOOLE_1),
            ("BOOLE-2", bitops::BOOLE_2),
            ("BOOLE-C1", bitops::BOOLE_C1),
            ("BOOLE-C2", bitops::BOOLE_C2),
            ("BOOLE-AND", bitops::BOOLE_AND),
            ("BOOLE-IOR", bitops::BOOLE_IOR),
            ("BOOLE-XOR", bitops::BOOLE_XOR),
            ("BOOLE-EQV", bitops::BOOLE_EQV),
            ("BOOLE-NAND", bitops::BOOLE_NAND),
            ("BOOLE-NOR", bitops::BOOLE_NOR),
            ("BOOLE-ANDC1", bitops::BOOLE_ANDC1),
            ("BOOLE-ANDC2", bitops::BOOLE_ANDC2),
            ("BOOLE-ORC1", bitops::BOOLE_ORC1),
            ("BOOLE-ORC2", bitops::BOOLE_ORC2),
            ("BOOLE-SET", bitops::BOOLE_SET),
        ] {
            let (symbol, _) = Package::from_word(*package).intern(ctx, runtime, name)?;
            set_symbol_constant(ctx, symbol, true)?;
            set_symbol_value(ctx, symbol, Word::fixnum(value))?;
        }
        Ok(())
    })?;
    Ok(())
}

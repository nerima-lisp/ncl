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

use ncl_object::{
    Arity, Builtin, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, LambdaList, ObjectError, Package, Parameter, ParameterType, Runtime,
    RustBuiltin, ThreadContext, Word, make_double, set_symbol_constant, set_symbol_special,
    set_symbol_value,
};

fn install(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &'static str,
    arity: u8,
    direct: bool,
    callback: RustBuiltin,
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
    runtime
        .register_builtin(
            ctx,
            identifier,
            BuiltinImplementation::direct(descriptor, callback),
        )
        .map(|_| ())
}

fn install_set(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    entries: &[(&'static str, u8, bool, RustBuiltin)],
) -> Result<(), ObjectError> {
    for &(name, arity, direct, callback) in entries {
        install(runtime, ctx, name, arity, direct, callback)?;
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
            ("NUMBERP", 1, true, arithmetic::typed_numberp),
            ("INTEGERP", 1, true, arithmetic::typed_integerp),
            ("RATIONALP", 1, true, arithmetic::typed_rationalp),
            ("FLOATP", 1, true, arithmetic::typed_floatp),
            ("REALP", 1, true, arithmetic::typed_realp),
            ("COMPLEXP", 1, true, arithmetic::typed_complexp),
            ("+", 0, false, arithmetic::typed_add),
            ("-", 0, false, arithmetic::typed_sub),
            ("*", 0, false, arithmetic::typed_mul),
            ("/", 0, false, arithmetic::typed_div),
            ("=", 0, false, arithmetic::typed_equal),
            ("EQ", 2, true, arithmetic::typed_eq),
            ("EQL", 2, true, arithmetic::typed_eql),
            ("/=", 0, false, arithmetic::typed_not_equal),
            ("<", 0, false, arithmetic::typed_less),
            (">", 0, false, arithmetic::typed_greater),
            ("<=", 0, false, arithmetic::typed_less_equal),
            (">=", 0, false, arithmetic::typed_greater_equal),
            ("MAX", 0, false, arithmetic::typed_max),
            ("MIN", 0, false, arithmetic::typed_min),
            ("1+", 1, true, arithmetic::typed_one_plus),
            ("1-", 1, true, arithmetic::typed_one_minus),
            ("ABS", 1, true, arithmetic::typed_abs),
            ("SIGNUM", 1, true, arithmetic::typed_signum),
            ("ZEROP", 1, true, arithmetic::typed_zerop),
            ("PLUSP", 1, true, arithmetic::typed_plusp),
            ("MINUSP", 1, true, arithmetic::typed_minusp),
            ("EVENP", 1, true, arithmetic::typed_evenp),
            ("ODDP", 1, true, arithmetic::typed_oddp),
            ("MOD", 2, true, remainder::typed_mod),
            ("REM", 2, true, remainder::typed_rem),
            ("GCD", 0, false, remainder::typed_gcd),
            ("LCM", 0, false, remainder::typed_lcm),
            ("ISQRT", 1, true, remainder::typed_isqrt),
            ("EXP", 1, true, transcendental::typed_exp),
            ("EXPT", 2, true, transcendental::typed_expt),
            ("SQRT", 1, true, transcendental::typed_sqrt),
            ("SIN", 1, true, transcendental::typed_sin),
            ("COS", 1, true, transcendental::typed_cos),
            ("TAN", 1, true, transcendental::typed_tan),
            ("ASIN", 1, true, transcendental::typed_asin),
            ("ACOS", 1, true, transcendental::typed_acos),
            ("SINH", 1, true, transcendental::typed_sinh),
            ("COSH", 1, true, transcendental::typed_cosh),
            ("TANH", 1, true, transcendental::typed_tanh),
            ("ASINH", 1, true, transcendental::typed_asinh),
            ("ACOSH", 1, true, transcendental::typed_acosh),
            ("ATANH", 1, true, transcendental::typed_atanh),
            ("COMPLEX", 2, true, complex::typed_complex),
            ("CONJUGATE", 1, true, complex::typed_conjugate),
            ("CIS", 1, true, complex::typed_cis),
            ("PHASE", 1, true, complex::typed_phase),
            ("REALPART", 1, true, complex::typed_realpart),
            ("IMAGPART", 1, true, complex::typed_imagpart),
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
            ("LOGAND", 0, false, bitops::typed_logand),
            ("LOGIOR", 0, false, bitops::typed_logior),
            ("LOGXOR", 0, false, bitops::typed_logxor),
            ("LOGNOT", 1, true, bitops::typed_lognot),
            ("LOGEQV", 0, false, bitops::typed_logeqv),
            ("LOGNAND", 2, true, bitops::typed_lognand),
            ("LOGNOR", 2, true, bitops::typed_lognor),
            ("LOGANDC1", 2, true, bitops::typed_logandc1),
            ("LOGANDC2", 2, true, bitops::typed_logandc2),
            ("LOGORC1", 2, true, bitops::typed_logorc1),
            ("LOGORC2", 2, true, bitops::typed_logorc2),
            ("LOGTEST", 2, true, bitops::typed_logtest),
            ("LOGBITP", 2, true, bitops::typed_logbitp),
            ("LOGCOUNT", 1, true, bitops::typed_logcount),
            ("INTEGER-LENGTH", 1, true, bitops::typed_integer_length),
            ("ASH", 2, true, bitops::typed_ash),
            ("BYTE", 2, true, bitops::typed_byte),
            ("BYTE-SIZE", 1, true, bitops::typed_byte_size),
            ("BYTE-POSITION", 1, true, bitops::typed_byte_position),
            ("LDB", 2, true, bitops::typed_ldb),
            ("DPB", 3, true, bitops::typed_dpb),
            ("LDB-TEST", 2, true, bitops::typed_ldb_test),
            ("MASK-FIELD", 2, true, bitops::typed_mask_field),
            ("DEPOSIT-FIELD", 3, true, bitops::typed_deposit_field),
            ("BOOLE", 3, true, bitops::typed_boole),
        ],
    )?;
    constants::register(&mut ctx, runtime)?;
    random::register(&mut ctx, runtime)?;
    let package = runtime
        .find_package(&ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    for name in ["PI", "MOST-POSITIVE-FIXNUM", "MOST-NEGATIVE-FIXNUM"] {
        let (symbol, _) = Package::from_word(package).intern(&mut ctx, runtime, name)?;
        set_symbol_constant(&mut ctx, symbol, true)?;
        let value = match name {
            "PI" => make_double(&mut ctx, runtime, std::f64::consts::PI)?.into(),
            "MOST-POSITIVE-FIXNUM" => Word::fixnum(i64::MAX >> 4),
            _ => Word::fixnum(i64::MIN >> 4),
        };
        set_symbol_value(&mut ctx, symbol, value)?;
    }
    for name in ["*", "+", "-", "/"] {
        let (symbol, _) = Package::from_word(package).intern(&mut ctx, runtime, name)?;
        set_symbol_special(&mut ctx, symbol, true)?;
        set_symbol_value(&mut ctx, symbol, Word::NIL)?;
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
        let (symbol, _) = Package::from_word(package).intern(&mut ctx, runtime, name)?;
        set_symbol_constant(&mut ctx, symbol, true)?;
        set_symbol_value(&mut ctx, symbol, Word::fixnum(value))?;
    }
    Ok(())
}

//! ANSI Common Lisp numeric builtins.

mod arithmetic;
mod bitops;

use ncl_object::{
    Arity, Builtin, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, LambdaList, ObjectError, Package, Parameter, ParameterType, Runtime,
    RustBuiltin, ThreadContext, Word, make_double, set_symbol_constant, set_symbol_value,
};

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
            ("NUMBERP", 1, true, arithmetic::typed_dispatch_numberp, None),
            (
                "INTEGERP",
                1,
                true,
                arithmetic::typed_dispatch_integerp,
                None,
            ),
            (
                "RATIONALP",
                1,
                true,
                arithmetic::typed_dispatch_rationalp,
                None,
            ),
            ("FLOATP", 1, true, arithmetic::typed_dispatch_floatp, None),
            ("REALP", 1, true, arithmetic::typed_dispatch_realp, None),
            (
                "COMPLEXP",
                1,
                true,
                arithmetic::typed_dispatch_complexp,
                None,
            ),
            (
                "+",
                0,
                false,
                arithmetic::typed_dispatch_add,
                Some(NativeEntry::Add),
            ),
            ("-", 0, false, arithmetic::typed_dispatch_sub, None),
            (
                "*",
                0,
                false,
                arithmetic::typed_dispatch_mul,
                Some(NativeEntry::Mul),
            ),
            ("/", 0, false, arithmetic::typed_dispatch_div, None),
            ("=", 0, false, arithmetic::typed_dispatch_equal, None),
            ("/=", 0, false, arithmetic::typed_dispatch_not_equal, None),
            ("<", 0, false, arithmetic::typed_dispatch_less, None),
            (">", 0, false, arithmetic::typed_dispatch_greater, None),
            ("<=", 0, false, arithmetic::typed_dispatch_less_equal, None),
            (
                ">=",
                0,
                false,
                arithmetic::typed_dispatch_greater_equal,
                None,
            ),
            ("MAX", 0, false, arithmetic::typed_dispatch_max, None),
            ("MIN", 0, false, arithmetic::typed_dispatch_min, None),
            ("1+", 1, true, arithmetic::typed_dispatch_one_plus, None),
            ("1-", 1, true, arithmetic::typed_dispatch_one_minus, None),
            ("ABS", 1, true, arithmetic::typed_dispatch_abs, None),
            ("SIGNUM", 1, true, arithmetic::typed_dispatch_signum, None),
            ("ZEROP", 1, true, arithmetic::typed_dispatch_zerop, None),
            ("PLUSP", 1, true, arithmetic::typed_dispatch_plusp, None),
            ("MINUSP", 1, true, arithmetic::typed_dispatch_minusp, None),
            ("EVENP", 1, true, arithmetic::typed_dispatch_evenp, None),
            ("ODDP", 1, true, arithmetic::typed_dispatch_oddp, None),
            ("FLOOR", 1, true, arithmetic::typed_dispatch_floor, None),
            ("CEILING", 1, true, arithmetic::typed_dispatch_ceiling, None),
            (
                "TRUNCATE",
                1,
                true,
                arithmetic::typed_dispatch_truncate,
                None,
            ),
            ("ROUND", 1, true, arithmetic::typed_dispatch_round, None),
            ("FFLOOR", 1, true, arithmetic::typed_dispatch_ffloor, None),
            (
                "FCEILING",
                1,
                true,
                arithmetic::typed_dispatch_fceiling,
                None,
            ),
            (
                "FTRUNCATE",
                1,
                true,
                arithmetic::typed_dispatch_ftruncate,
                None,
            ),
            ("FROUND", 1, true, arithmetic::typed_dispatch_fround, None),
            ("MOD", 2, true, arithmetic::typed_dispatch_mod, None),
            ("REM", 2, true, arithmetic::typed_dispatch_rem, None),
            ("GCD", 0, false, arithmetic::typed_dispatch_gcd, None),
            ("LCM", 0, false, arithmetic::typed_dispatch_lcm, None),
            ("ISQRT", 1, true, arithmetic::typed_dispatch_isqrt, None),
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

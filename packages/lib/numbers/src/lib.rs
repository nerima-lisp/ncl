//! ANSI Common Lisp numeric builtins.

mod arithmetic;
mod bitops;

use ncl_object::{Builtin, BuiltinImplementation, ObjectError, Package, Runtime, ThreadContext, Word, MultipleValues, RustBuiltin, make_double, set_symbol_constant, set_symbol_value};

fn install(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, arity: u8, direct: bool, callback: RustBuiltin) -> Result<(), ObjectError> {
    runtime.register_builtin(ctx, "COMMON-LISP", name, BuiltinImplementation::direct(Builtin { arity, direct, lambda_list: "" }, callback)).map(|_| ())
}

fn install_set(runtime: &Runtime, ctx: &mut ThreadContext, entries: &[(&str, u8, bool, RustBuiltin)]) -> Result<(), ObjectError> {
    for &(name, arity, direct, callback) in entries { install(runtime, ctx, name, arity, direct, callback)?; }
    Ok(())
}

/// Register numeric predicates, arithmetic, rounding, and integer operations.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    install_set(runtime, &mut ctx, &[
        ("NUMBERP", 1, true, arithmetic::dispatch_numberp), ("INTEGERP", 1, true, arithmetic::dispatch_integerp), ("RATIONALP", 1, true, arithmetic::dispatch_rationalp), ("FLOATP", 1, true, arithmetic::dispatch_floatp), ("REALP", 1, true, arithmetic::dispatch_realp), ("COMPLEXP", 1, true, arithmetic::dispatch_complexp),
        ("+", 0, false, arithmetic::dispatch_add), ("-", 0, false, arithmetic::dispatch_sub), ("*", 0, false, arithmetic::dispatch_mul), ("/", 0, false, arithmetic::dispatch_div), ("=", 0, false, arithmetic::dispatch_equal), ("/=", 0, false, arithmetic::dispatch_not_equal), ("<", 0, false, arithmetic::dispatch_less), (">", 0, false, arithmetic::dispatch_greater), ("<=", 0, false, arithmetic::dispatch_less_equal), (">=", 0, false, arithmetic::dispatch_greater_equal), ("MAX", 0, false, arithmetic::dispatch_max), ("MIN", 0, false, arithmetic::dispatch_min),
        ("1+", 1, true, arithmetic::dispatch_one_plus), ("1-", 1, true, arithmetic::dispatch_one_minus), ("ABS", 1, true, arithmetic::dispatch_abs), ("SIGNUM", 1, true, arithmetic::dispatch_signum), ("ZEROP", 1, true, arithmetic::dispatch_zerop), ("PLUSP", 1, true, arithmetic::dispatch_plusp), ("MINUSP", 1, true, arithmetic::dispatch_minusp), ("EVENP", 1, true, arithmetic::dispatch_evenp), ("ODDP", 1, true, arithmetic::dispatch_oddp),
        ("FLOOR", 1, true, arithmetic::dispatch_floor), ("CEILING", 1, true, arithmetic::dispatch_ceiling), ("TRUNCATE", 1, true, arithmetic::dispatch_truncate), ("ROUND", 1, true, arithmetic::dispatch_round), ("FFLOOR", 1, true, arithmetic::dispatch_ffloor), ("FCEILING", 1, true, arithmetic::dispatch_fceiling), ("FTRUNCATE", 1, true, arithmetic::dispatch_ftruncate), ("FROUND", 1, true, arithmetic::dispatch_fround), ("MOD", 2, true, arithmetic::dispatch_mod), ("REM", 2, true, arithmetic::dispatch_rem), ("GCD", 0, false, arithmetic::dispatch_gcd), ("LCM", 0, false, arithmetic::dispatch_lcm), ("ISQRT", 1, true, arithmetic::dispatch_isqrt),
    ])?;
    install_set(runtime, &mut ctx, &[
        ("LOGAND", 0, false, bitops::logand), ("LOGIOR", 0, false, bitops::logior), ("LOGXOR", 0, false, bitops::logxor), ("LOGNOT", 1, true, bitops::lognot), ("LOGEQV", 0, false, bitops::logeqv), ("LOGNAND", 2, true, bitops::lognand), ("LOGNOR", 2, true, bitops::lognor), ("LOGANDC1", 2, true, bitops::logandc1), ("LOGANDC2", 2, true, bitops::logandc2), ("LOGORC1", 2, true, bitops::logorc1), ("LOGORC2", 2, true, bitops::logorc2), ("LOGTEST", 2, true, bitops::logtest), ("LOGBITP", 2, true, bitops::logbitp), ("LOGCOUNT", 1, true, bitops::logcount), ("INTEGER-LENGTH", 1, true, bitops::integer_length), ("ASH", 2, true, bitops::ash), ("BYTE", 2, true, bitops::byte), ("BYTE-SIZE", 1, true, bitops::byte_size), ("BYTE-POSITION", 1, true, bitops::byte_position), ("LDB", 2, true, bitops::ldb), ("DPB", 3, true, bitops::dpb), ("LDB-TEST", 2, true, bitops::ldb_test), ("MASK-FIELD", 2, true, bitops::mask_field), ("DEPOSIT-FIELD", 3, true, bitops::deposit_field), ("BOOLE", 3, true, bitops::boole),
    ])?;
    let package = runtime.find_package(&ctx, "COMMON-LISP").ok_or(ObjectError::PackageConflict)?;
    for name in ["PI", "MOST-POSITIVE-FIXNUM", "MOST-NEGATIVE-FIXNUM"] {
        let (symbol, _) = Package::from(package).intern(&mut ctx, runtime, name)?;
        set_symbol_constant(&mut ctx, symbol, true)?;
        let value = match name { "PI" => make_double(&mut ctx, runtime, std::f64::consts::PI)?.into(), "MOST-POSITIVE-FIXNUM" => Word::fixnum(i64::MAX >> 4), _ => Word::fixnum(i64::MIN >> 4) };
        set_symbol_value(&mut ctx, symbol, value)?;
    }
    Ok(())
}

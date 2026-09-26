//! Common Lisp numeric constants owned by the numbers library.

use ncl_object::{
    ObjectError, Package, Runtime, ThreadContext, Word, make_double, set_symbol_constant,
    set_symbol_value,
};

use crate::{MOST_NEGATIVE_FIXNUM, MOST_POSITIVE_FIXNUM};

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

/// Register implementation-independent numeric constants supported by NCL.
pub fn register(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    let package = runtime
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    set_constant(
        ctx,
        runtime,
        package,
        "MOST-POSITIVE-FIXNUM",
        Word::fixnum(MOST_POSITIVE_FIXNUM),
    )?;
    set_constant(
        ctx,
        runtime,
        package,
        "MOST-NEGATIVE-FIXNUM",
        Word::fixnum(MOST_NEGATIVE_FIXNUM),
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

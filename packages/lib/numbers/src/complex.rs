//! Complex-number accessors and constructors.

use core::cell::Cell;

use ncl_object::{
    classify_object, complex_imag, complex_real, double_value, make_complex, make_double,
    BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
};
use ncl_sys::{RootSlot, RootToken};

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

fn with_roots<T>(
    ctx: &mut ThreadContext,
    values: &[Word],
    f: impl FnOnce(&mut ThreadContext, &[RootSlot<'_>]) -> Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    let mut cells: Vec<Cell<Word>> = values.iter().copied().map(Cell::new).collect();
    let mut tokens: Vec<RootToken> = Vec::with_capacity(cells.len());
    for cell in &mut cells {
        tokens.push(ncl_object::push_root(ctx, cell.get_mut()));
    }
    let slots: Vec<RootSlot<'_>> = cells.iter().map(RootSlot::new).collect();
    let result = f(ctx, &slots);
    for token in tokens.into_iter().rev() {
        if !ncl_object::pop_root(ctx, token) {
            return Err(ObjectError::Layout);
        }
    }
    result
}

fn fixnum_to_f64(value: i64) -> Result<f64, ObjectError> {
    value
        .to_string()
        .parse::<f64>()
        .map_err(|_| ObjectError::Layout)
}

fn real(ctx: &ThreadContext, value: Word) -> Result<f64, ObjectError> {
    match classify_object(ctx, value) {
        ObjectRef::Fixnum(value) => fixnum_to_f64(value),
        ObjectRef::DoubleFloat(value) => {
            double_value(ctx, ncl_object::DoubleFloat::from_word(value))
        }
        _ => Err(ObjectError::TypeError),
    }
}

fn component(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Word,
    imaginary: bool,
) -> Result<Word, ObjectError> {
    let ObjectRef::Complex(value) = classify_object(ctx, value) else {
        return make_double(
            ctx,
            runtime,
            if imaginary { 0.0 } else { real(ctx, value)? },
        )
        .map(Into::into);
    };
    let value = if imaginary {
        complex_imag(ctx, ncl_object::Complex::from_word(value))?
    } else {
        complex_real(ctx, ncl_object::Complex::from_word(value))?
    };
    Ok(value)
}

pub fn typed_complex(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let values = [args.required(0)?, args.required(1)?];
    with_roots(ctx, &values, |ctx, values| {
        let real_value = real(ctx, *values[0])?;
        let imag_value = real(ctx, *values[1])?;
        if imag_value == 0.0 {
            return Ok(*values[0]);
        }
        let mut real = make_double(ctx, runtime, real_value)?.into();
        with_root(ctx, &mut real, |ctx, real| {
            let imag = make_double(ctx, runtime, imag_value)?.into();
            make_complex(ctx, runtime, *real, imag).map(Into::into)
        })
    })
}

pub fn typed_conjugate(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut value = args.required(0)?;
    with_root(ctx, &mut value, |ctx, value| {
        if let ObjectRef::Complex(value) = classify_object(ctx, *value) {
            let object = ncl_object::Complex::from_word(value);
            let mut real_word = complex_real(ctx, object)?;
            let imag_value = -real(ctx, complex_imag(ctx, object)?)?;
            with_root(ctx, &mut real_word, |ctx, real_word| {
                let imag = make_double(ctx, runtime, imag_value)?.into();
                make_complex(ctx, runtime, *real_word, imag).map(Into::into)
            })
        } else {
            real(ctx, *value)?;
            Ok(*value)
        }
    })
}

pub fn typed_realpart(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    component(ctx, runtime, args.required(0)?, false)
}
pub fn typed_imagpart(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    component(ctx, runtime, args.required(0)?, true)
}

pub fn typed_phase(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = args.required(0)?;
    let (real_value, imag_value) = match classify_object(ctx, value) {
        ObjectRef::Complex(value) => {
            let object = ncl_object::Complex::from_word(value);
            (
                real(ctx, complex_real(ctx, object)?)?,
                real(ctx, complex_imag(ctx, object)?)?,
            )
        }
        _ => (real(ctx, value)?, 0.0),
    };
    make_double(ctx, runtime, imag_value.atan2(real_value)).map(Into::into)
}

pub fn typed_cis(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let angle = real(ctx, args.required(0)?)?;
    let mut real = make_double(ctx, runtime, angle.cos())?.into();
    with_root(ctx, &mut real, |ctx, real| {
        let imag = make_double(ctx, runtime, angle.sin())?.into();
        make_complex(ctx, runtime, *real, imag).map(Into::into)
    })
}

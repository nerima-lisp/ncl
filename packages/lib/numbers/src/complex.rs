//! Complex-number accessors and constructors.

use ncl_object::{
    classify_object, complex_imag, complex_real, double_value, make_complex, make_double,
    BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
};

fn real(ctx: &ThreadContext, value: Word) -> Result<f64, ObjectError> {
    match classify_object(ctx, value) {
        ObjectRef::Fixnum(value) => Ok(value as f64),
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
    match classify_object(ctx, value) {
        ObjectRef::Complex(value) => {
            if imaginary {
                Ok(complex_imag(ctx, ncl_object::Complex::from_word(value))?)
            } else {
                Ok(complex_real(ctx, ncl_object::Complex::from_word(value))?)
            }
        }
        _ => {
            let value = if imaginary { 0.0 } else { real(ctx, value)? };
            make_double(ctx, runtime, value).map(Into::into)
        }
    }
}

pub fn typed_complex(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let real = args.required(0)?;
    let real_value = real(ctx, real)?;
    let imag = args.required(1)?;
    let imag_value = real(ctx, imag)?;
    if imag_value == 0.0 {
        return Ok(real);
    }
    let real = make_double(ctx, runtime, real_value)?.into();
    let imag = make_double(ctx, runtime, imag_value)?.into();
    make_complex(ctx, runtime, real, imag).map(Into::into)
}

pub fn typed_conjugate(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = args.required(0)?;
    match classify_object(ctx, value) {
        ObjectRef::Complex(value) => {
            let real = complex_real(ctx, ncl_object::Complex::from_word(value))?;
            let imag = complex_imag(ctx, ncl_object::Complex::from_word(value))?;
            let imag_value = -real(ctx, imag)?;
            let imag = make_double(ctx, runtime, imag_value)?.into();
            make_complex(ctx, runtime, real, imag).map(Into::into)
        }
        _ => {
            real(ctx, value)?;
            Ok(value)
        }
    }
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
    let (real, imag) = match classify_object(ctx, value) {
        ObjectRef::Complex(value) => {
            let value = ncl_object::Complex::from_word(value);
            (
                real(ctx, complex_real(ctx, value)?)?,
                real(ctx, complex_imag(ctx, value)?)?,
            )
        }
        _ => (real(ctx, value)?, 0.0),
    };
    make_double(ctx, runtime, imag.atan2(real)).map(Into::into)
}

pub fn typed_cis(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let angle = real(ctx, args.required(0)?)?;
    let real = make_double(ctx, runtime, angle.cos())?.into();
    let imag = make_double(ctx, runtime, angle.sin())?.into();
    make_complex(ctx, runtime, real, imag).map(Into::into)
}

//! Complex-number accessors and constructors.

use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
    classify_object, complex_imag, complex_real, double_value, make_complex, make_double,
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
    let value = match classify_object(ctx, value) {
        ObjectRef::Complex(value) => {
            if imaginary {
                complex_imag(ctx, ncl_object::Complex::from_word(value))?
            } else {
                complex_real(ctx, ncl_object::Complex::from_word(value))?
            }
        }
        _ => {
            return make_double(
                ctx,
                runtime,
                if imaginary { 0.0 } else { real(ctx, value)? },
            )
            .map(Into::into);
        }
    };
    Ok(value)
}

pub fn typed_complex(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let real_word = args.required(0)?;
    let real_value = real(ctx, real_word)?;
    let imag_value = real(ctx, args.required(1)?)?;
    if imag_value == 0.0 {
        return Ok(real_word);
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
            let object = ncl_object::Complex::from_word(value);
            let real_word = complex_real(ctx, object)?;
            let imag_value = -real(ctx, complex_imag(ctx, object)?)?;
            let imag = make_double(ctx, runtime, imag_value)?.into();
            make_complex(ctx, runtime, real_word, imag).map(Into::into)
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
    let real = make_double(ctx, runtime, angle.cos())?.into();
    let imag = make_double(ctx, runtime, angle.sin())?.into();
    make_complex(ctx, runtime, real, imag).map(Into::into)
}

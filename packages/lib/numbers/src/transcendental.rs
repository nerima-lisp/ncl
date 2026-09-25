//! Transcendental numeric builtins.

use ncl_object::{
    bignum_limbs, bignum_sign, classify_object, complex_imag, complex_real, double_value,
    make_complex, make_double, ratio_denominator, ratio_numerator, BuiltinArgs, MultipleValues,
    ObjectError, ObjectRef, Runtime, ThreadContext, Word,
};

#[derive(Clone, Copy)]
enum Number {
    Real(f64),
    Complex(f64, f64),
}

fn number(ctx: &ThreadContext, value: Word) -> Result<Number, ObjectError> {
    match classify_object(ctx, value) {
        ObjectRef::Fixnum(value) => Ok(Number::Real(value as f64)),
        ObjectRef::Bignum(value) => Ok(Number::Real(integer(ctx, value.into())? as f64)),
        ObjectRef::Ratio(value) => {
            let numerator = integer(
                ctx,
                ratio_numerator(ctx, ncl_object::Ratio::from_word(value))?,
            )?;
            let denominator = integer(
                ctx,
                ratio_denominator(ctx, ncl_object::Ratio::from_word(value))?,
            )?;
            Ok(Number::Real(numerator as f64 / denominator as f64))
        }
        ObjectRef::DoubleFloat(value) => Ok(Number::Real(double_value(
            ctx,
            ncl_object::DoubleFloat::from_word(value),
        )?)),
        ObjectRef::Complex(value) => {
            let real = number(
                ctx,
                complex_real(ctx, ncl_object::Complex::from_word(value))?,
            )?;
            let imag = number(
                ctx,
                complex_imag(ctx, ncl_object::Complex::from_word(value))?,
            )?;
            Ok(Number::Complex(real.real(), imag.real()))
        }
        _ => Err(ObjectError::TypeError),
    }
}

fn integer(ctx: &ThreadContext, value: Word) -> Result<i128, ObjectError> {
    if let Some(value) = value.as_fixnum() {
        return Ok(i128::from(value));
    }
    let ObjectRef::Bignum(value) = classify_object(ctx, value) else {
        return Err(ObjectError::TypeError);
    };
    let limbs = bignum_limbs(ctx, ncl_object::Bignum::from_word(value))?;
    let magnitude = limbs
        .into_iter()
        .enumerate()
        .try_fold(0_i128, |sum, (index, limb)| {
            sum.checked_add(
                i128::from(limb)
                    .checked_shl((index * 32) as u32)
                    .ok_or(ObjectError::Layout)?,
            )
            .ok_or(ObjectError::Layout)
        })?;
    if bignum_sign(ctx, ncl_object::Bignum::from_word(value))? {
        Ok(-magnitude)
    } else {
        Ok(magnitude)
    }
}

impl Number {
    fn real(self) -> f64 {
        match self {
            Self::Real(value) | Self::Complex(value, _) => value,
        }
    }

    fn pair(self) -> (f64, f64) {
        match self {
            Self::Real(value) => (value, 0.0),
            Self::Complex(real, imag) => (real, imag),
        }
    }

    fn is_complex(self) -> bool {
        matches!(self, Self::Complex(_, _))
    }
}

fn output(ctx: &mut ThreadContext, runtime: &Runtime, value: Number) -> Result<Word, ObjectError> {
    match value {
        Number::Real(value) => make_double(ctx, runtime, value).map(Into::into),
        Number::Complex(real, imag) => {
            let real = make_double(ctx, runtime, real)?.into();
            let imag = make_double(ctx, runtime, imag)?.into();
            make_complex(ctx, runtime, real, imag).map(Into::into)
        }
    }
}

fn complex_mul(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}

fn complex_div(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let denominator = b.0 * b.0 + b.1 * b.1;
    (
        (a.0 * b.0 + a.1 * b.1) / denominator,
        (a.1 * b.0 - a.0 * b.1) / denominator,
    )
}

fn complex_exp((real, imag): (f64, f64)) -> (f64, f64) {
    let scale = real.exp();
    (scale * imag.cos(), scale * imag.sin())
}

fn complex_log((real, imag): (f64, f64)) -> (f64, f64) {
    (real.hypot(imag).ln(), imag.atan2(real))
}

fn complex_sqrt((real, imag): (f64, f64)) -> (f64, f64) {
    let magnitude = real.hypot(imag);
    let root = ((magnitude + real) / 2.0).sqrt();
    let imag_root = ((magnitude - real) / 2.0).sqrt().copysign(imag);
    (root, imag_root)
}

fn complex_sin((real, imag): (f64, f64)) -> (f64, f64) {
    (real.sin() * imag.cosh(), real.cos() * imag.sinh())
}

fn complex_cos((real, imag): (f64, f64)) -> (f64, f64) {
    (real.cos() * imag.cosh(), -real.sin() * imag.sinh())
}

fn complex_asin(value: (f64, f64)) -> (f64, f64) {
    let iz = (-value.1, value.0);
    let root = complex_sqrt((
        1.0 - value.0 * value.0 + value.1 * value.1,
        -2.0 * value.0 * value.1,
    ));
    let log = complex_log((iz.0 + root.0, iz.1 + root.1));
    (log.1, -log.0)
}

fn complex_acos(value: (f64, f64)) -> (f64, f64) {
    let asin = complex_asin(value);
    (std::f64::consts::FRAC_PI_2 - asin.0, -asin.1)
}

fn complex_atan(value: (f64, f64)) -> (f64, f64) {
    let left = complex_log((1.0 + value.1, -value.0));
    let right = complex_log((1.0 - value.1, value.0));
    ((left.1 - right.1) / 2.0, (right.0 - left.0) / 2.0)
}

fn unary(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    operation: impl Fn(Number) -> Number,
) -> Result<Word, ObjectError> {
    let value = number(ctx, args.required(0)?)?;
    output(ctx, runtime, operation(value))
}

fn binary(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    operation: impl Fn(Number, Number) -> Number,
) -> Result<Word, ObjectError> {
    let left = number(ctx, args.required(0)?)?;
    let right = number(ctx, args.required(1)?)?;
    let value = operation(left, right);
    output(ctx, runtime, value)
}

pub fn typed_exp(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    unary(ctx, runtime, args, |value| match value {
        Number::Real(value) => Number::Real(value.exp()),
        Number::Complex(_, _) => {
            let pair = complex_exp(value.pair());
            Number::Complex(pair.0, pair.1)
        }
    })
}

pub fn typed_expt(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    binary(ctx, runtime, args, |base, exponent| {
        let log = complex_log(base.pair());
        let exponent = exponent.pair();
        let result = complex_exp((
            log.0 * exponent.0 - log.1 * exponent.1,
            log.0 * exponent.1 + log.1 * exponent.0,
        ));
        if !base.is_complex() && !exponent.1.is_normal() {
            Number::Real(result.0)
        } else {
            Number::Complex(result.0, result.1)
        }
    })
}

pub fn typed_log(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = number(ctx, args.required(0)?)?;
    let mut result = complex_log(value.pair());
    if let Some(base) = args.get(1) {
        let base = number(ctx, base)?.real();
        result.0 /= base.ln();
        result.1 /= base.ln();
    }
    if !value.is_complex() && result.1 == 0.0 {
        output(ctx, runtime, Number::Real(result.0))
    } else {
        output(ctx, runtime, Number::Complex(result.0, result.1))
    }
}

pub fn typed_sqrt(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    unary(ctx, runtime, args, |value| {
        let result = complex_sqrt(value.pair());
        if !value.is_complex() && result.1 == 0.0 {
            Number::Real(result.0)
        } else {
            Number::Complex(result.0, result.1)
        }
    })
}

macro_rules! unary_complex {
    ($name:ident, $operation:expr) => {
        pub fn $name(
            ctx: &mut ThreadContext,
            runtime: &Runtime,
            args: &BuiltinArgs<'_>,
            _: &mut MultipleValues,
        ) -> Result<Word, ObjectError> {
            unary(ctx, runtime, args, |value| {
                let result = $operation(value.pair());
                if value.is_complex() {
                    Number::Complex(result.0, result.1)
                } else {
                    Number::Real(result.0)
                }
            })
        }
    };
}

unary_complex!(typed_sin, complex_sin);
unary_complex!(typed_cos, complex_cos);
unary_complex!(typed_asin, complex_asin);
unary_complex!(typed_acos, complex_acos);

pub fn typed_atan(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if let Some(x) = args.get(1) {
        let y = number(ctx, args.required(0)?)?.real();
        let x = number(ctx, x)?.real();
        return output(ctx, runtime, Number::Real(y.atan2(x)));
    }
    unary(ctx, runtime, args, |value| {
        let result = complex_atan(value.pair());
        if value.is_complex() {
            Number::Complex(result.0, result.1)
        } else {
            Number::Real(result.0)
        }
    })
}

pub fn typed_tan(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    unary(ctx, runtime, args, |value| {
        let result = complex_div(complex_sin(value.pair()), complex_cos(value.pair()));
        if value.is_complex() {
            Number::Complex(result.0, result.1)
        } else {
            Number::Real(result.0)
        }
    })
}

unary_complex!(typed_sinh, |(real, imag): (f64, f64)| (
    real.sinh() * imag.cos(),
    real.cosh() * imag.sin()
));
unary_complex!(typed_cosh, |(real, imag): (f64, f64)| (
    real.cosh() * imag.cos(),
    real.sinh() * imag.sin()
));
unary_complex!(typed_tanh, |value: (f64, f64)| complex_div(
    (
        value.0.sinh() * value.1.cos(),
        value.0.cosh() * value.1.sin()
    ),
    (
        value.0.cosh() * value.1.cos(),
        value.0.sinh() * value.1.sin()
    )
));
unary_complex!(typed_acosh, |value: (f64, f64)| complex_log(complex_add(
    value,
    complex_mul(
        complex_sqrt((value.0 + 1.0, value.1)),
        complex_sqrt((value.0 - 1.0, value.1))
    )
)));
unary_complex!(typed_asinh, |value: (f64, f64)| complex_log(complex_add(
    value,
    complex_sqrt((
        value.0 * value.0 - value.1 * value.1 + 1.0,
        2.0 * value.0 * value.1
    ))
)));
unary_complex!(typed_atanh, |value: (f64, f64)| {
    let result = complex_log(complex_div(
        (1.0 + value.0, value.1),
        (1.0 - value.0, -value.1),
    ));
    (result.0 / 2.0, result.1 / 2.0)
});

fn complex_add(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 + b.0, a.1 + b.1)
}

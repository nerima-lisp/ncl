//! Transcendental numeric builtins.

use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
    bignum_limbs, bignum_sign, classify_object, complex_imag, complex_real, double_value,
    make_complex, make_double, ratio_denominator, ratio_numerator,
};

#[derive(Clone, Copy)]
enum Number {
    Real(f64),
    Complex(f64, f64),
}

fn integer(ctx: &ThreadContext, value: Word) -> Result<i128, ObjectError> {
    if let Some(value) = value.as_fixnum() {
        return Ok(i128::from(value));
    }
    let ObjectRef::Bignum(value) = classify_object(ctx, value) else {
        return Err(ObjectError::TypeError);
    };
    let magnitude = bignum_limbs(ctx, ncl_object::Bignum::from_word(value))?
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
            let value = ncl_object::Complex::from_word(value);
            let real = number(ctx, complex_real(ctx, value)?)?.real();
            let imag = number(ctx, complex_imag(ctx, value)?)?.real();
            Ok(Number::Complex(real, imag))
        }
        _ => Err(ObjectError::TypeError),
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

fn add(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 + b.0, a.1 + b.1)
}

fn mul(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}

fn div(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let scale = b.0.hypot(b.1);
    let (br, bi) = (b.0 / scale, b.1 / scale);
    let denominator = br * br + bi * bi;
    (
        (a.0 / scale * br + a.1 / scale * bi) / denominator,
        (a.1 / scale * br - a.0 / scale * bi) / denominator,
    )
}

fn exp(value: (f64, f64)) -> (f64, f64) {
    let scale = value.0.exp();
    (scale * value.1.cos(), scale * value.1.sin())
}

fn log(value: (f64, f64)) -> (f64, f64) {
    (value.0.hypot(value.1).ln(), value.1.atan2(value.0))
}

fn sqrt((real, imag): (f64, f64)) -> (f64, f64) {
    if imag == 0.0 {
        if real < 0.0 {
            return (0.0, (-real).sqrt().copysign(imag));
        }
        return (real.sqrt(), imag.copysign(0.0));
    }
    let magnitude = real.hypot(imag);
    let root = ((magnitude + real) / 2.0).sqrt();
    let imag_root = ((magnitude - real) / 2.0).sqrt().copysign(imag);
    (root, imag_root)
}

fn sin((real, imag): (f64, f64)) -> (f64, f64) {
    (real.sin() * imag.cosh(), real.cos() * imag.sinh())
}

fn cos((real, imag): (f64, f64)) -> (f64, f64) {
    (real.cos() * imag.cosh(), -real.sin() * imag.sinh())
}

fn asin(value: (f64, f64)) -> (f64, f64) {
    let root = sqrt((
        1.0 - value.0 * value.0 + value.1 * value.1,
        -2.0 * value.0 * value.1,
    ));
    let result = log(add((-value.1, value.0), root));
    (result.1, -result.0)
}

fn acos(value: (f64, f64)) -> (f64, f64) {
    let result = asin(value);
    (std::f64::consts::FRAC_PI_2 - result.0, -result.1)
}

fn atan(value: (f64, f64)) -> (f64, f64) {
    let left = log((1.0 + value.1, -value.0));
    let right = log((1.0 - value.1, value.0));
    ((right.1 - left.1) / 2.0, (right.0 - left.0) / 2.0)
}

fn unary(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    operation: impl Fn(Number) -> Number,
) -> Result<Word, ObjectError> {
    output(ctx, runtime, operation(number(ctx, args.required(0)?)?))
}

fn binary(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    operation: impl Fn(Number, Number) -> Number,
) -> Result<Word, ObjectError> {
    let left = number(ctx, args.required(0)?)?;
    let right = number(ctx, args.required(1)?)?;
    output(ctx, runtime, operation(left, right))
}

pub fn typed_exp(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    unary(ctx, runtime, args, |value| match value {
        Number::Real(value) => Number::Real(value.exp()),
        Number::Complex(real, imag) => {
            let (real, imag) = exp((real, imag));
            Number::Complex(real, imag)
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
        let value = exp(mul(log(base.pair()), exponent.pair()));
        if !base.is_complex() && !exponent.is_complex() && value.1 == 0.0 {
            Number::Real(value.0)
        } else {
            Number::Complex(value.0, value.1)
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
    let mut result = log(value.pair());
    if let Some(base) = args.get(1) {
        let base = number(ctx, base)?.real();
        let divisor = base.ln();
        result.0 /= divisor;
        result.1 /= divisor;
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
        let result = sqrt(value.pair());
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

unary_complex!(typed_sin, sin);
unary_complex!(typed_cos, cos);
unary_complex!(typed_asin, asin);
unary_complex!(typed_acos, acos);

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
        let result = atan(value.pair());
        if value.is_complex() {
            Number::Complex(result.0, result.1)
        } else {
            Number::Real(result.0)
        }
    })
}

unary_complex!(typed_tan, |value: (f64, f64)| div(sin(value), cos(value)));
unary_complex!(typed_sinh, |(real, imag): (f64, f64)| (
    real.sinh() * imag.cos(),
    real.cosh() * imag.sin()
));
unary_complex!(typed_cosh, |(real, imag): (f64, f64)| (
    real.cosh() * imag.cos(),
    real.sinh() * imag.sin()
));
unary_complex!(typed_tanh, |value: (f64, f64)| div(
    (
        value.0.sinh() * value.1.cos(),
        value.0.cosh() * value.1.sin()
    ),
    (
        value.0.cosh() * value.1.cos(),
        value.0.sinh() * value.1.sin()
    )
));
unary_complex!(typed_acosh, |value: (f64, f64)| log(add(
    value,
    mul(
        sqrt((value.0 + 1.0, value.1)),
        sqrt((value.0 - 1.0, value.1))
    )
)));
unary_complex!(typed_asinh, |value: (f64, f64)| log(add(
    value,
    sqrt((
        value.0 * value.0 - value.1 * value.1 + 1.0,
        2.0 * value.0 * value.1
    ))
)));
unary_complex!(typed_atanh, |value: (f64, f64)| {
    let result = log(div((1.0 + value.0, value.1), (1.0 - value.0, -value.1)));
    (result.0 / 2.0, result.1 / 2.0)
});

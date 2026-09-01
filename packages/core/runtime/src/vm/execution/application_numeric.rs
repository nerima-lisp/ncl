#[allow(clippy::wildcard_imports)]
use super::*;

pub fn execute_numeric_unary_instruction(
    operation: &str,
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    let value = stack
        .pop()
        .ok_or_else(|| invalid("unary numeric operation has too few stack values", span))?;
    let result = match operation {
        "1+" => crate::builtins::increment(&[value]),
        "1-" => crate::builtins::decrement(&[value]),
        "ABS" => crate::builtins::absolute(&[value]),
        "SIGNUM" => crate::builtins::signum(&[value]),
        "ZEROP" => crate::builtins::zerop(&[value]),
        "PLUSP" => crate::builtins::plusp(&[value]),
        "MINUSP" => crate::builtins::minusp(&[value]),
        "EVENP" => crate::builtins::evenp(&[value]),
        "ODDP" => crate::builtins::oddp(&[value]),
        "LOGNOT" => crate::builtins::lognot(&[value]),
        "LOGCOUNT" => crate::builtins::logcount(&[value]),
        "INTEGER-LENGTH" => crate::builtins::integer_length(&[value]),
        "ISQRT" => crate::builtins::integer_square_root_builtin(&[value]),
        "SQRT" => crate::builtins::square_root(&[value]),
        "SIN" => crate::builtins::sine(&[value]),
        "COS" => crate::builtins::cosine(&[value]),
        "CIS" => crate::builtins::cis(&[value]),
        "TAN" => crate::builtins::tangent(&[value]),
        "EXP" => crate::builtins::exponential(&[value]),
        "ASIN" => crate::builtins::arc_sine(&[value]),
        "ACOS" => crate::builtins::arc_cosine(&[value]),
        "SINH" => crate::builtins::hyperbolic_sine(&[value]),
        "COSH" => crate::builtins::hyperbolic_cosine(&[value]),
        "TANH" => crate::builtins::hyperbolic_tangent(&[value]),
        "REALPART" => crate::builtins::real_part(&[value]),
        "IMAGPART" => crate::builtins::imaginary_part(&[value]),
        "CONJUGATE" => crate::builtins::conjugate(&[value]),
        "PHASE" => crate::builtins::phase(&[value]),
        "RATIONAL" => crate::builtins::rational(&[value]),
        "RATIONALIZE" => crate::builtins::rationalize(&[value]),
        "NUMERATOR" => crate::builtins::numerator(&[value]),
        "DENOMINATOR" => crate::builtins::denominator(&[value]),
        _ => Err(invalid("unknown unary numeric operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_numeric_rounding_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("numeric rounding has too few stack values", span));
    }
    let start = stack.len() - argument_count;
    let arguments = stack.drain(start..).collect::<Vec<_>>();
    let result = match operation {
        "FLOOR" => crate::builtins::floor(&arguments),
        "CEILING" => crate::builtins::ceiling(&arguments),
        "TRUNCATE" => crate::builtins::truncate(&arguments),
        "ROUND" => crate::builtins::round(&arguments),
        _ => Err(invalid("unknown numeric rounding operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_numeric_comparison_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("numeric comparison has too few stack values", span));
    }
    let start = stack.len() - argument_count;
    let arguments = stack.drain(start..).collect::<Vec<_>>();
    let result = match operation {
        "=" => crate::builtins::numeric_equal(&arguments),
        "/=" => crate::builtins::numeric_not_equal(&arguments),
        "<" => crate::builtins::less_than(&arguments),
        ">" => crate::builtins::greater_than(&arguments),
        "<=" => crate::builtins::less_equal(&arguments),
        ">=" => crate::builtins::greater_equal(&arguments),
        _ => Err(invalid("unknown numeric comparison operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_numeric_fold_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("numeric fold has too few stack values", span));
    }
    let start = stack.len() - argument_count;
    let arguments = stack.drain(start..).collect::<Vec<_>>();
    let result = match operation {
        "MIN" => crate::builtins::minimum(&arguments),
        "MAX" => crate::builtins::maximum(&arguments),
        "GCD" => crate::builtins::greatest_common_divisor(&arguments),
        "LCM" => crate::builtins::least_common_multiple(&arguments),
        "LOGAND" => crate::builtins::logand(&arguments),
        "LOGIOR" => crate::builtins::logior(&arguments),
        "LOGXOR" => crate::builtins::logxor(&arguments),
        _ => Err(invalid("unknown numeric fold operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_numeric_binary_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    span: Span,
) -> Result<(), RuntimeError> {
    let right = stack
        .pop()
        .ok_or_else(|| invalid("numeric binary operation has too few stack values", span))?;
    let left = stack
        .pop()
        .ok_or_else(|| invalid("numeric binary operation has too few stack values", span))?;
    let result = match operation {
        "MOD" => crate::builtins::modulo(&[left, right]),
        "REM" => crate::builtins::remainder(&[left, right]),
        "ASH" => crate::builtins::arithmetic_shift(&[left, right]),
        "LOGTEST" => crate::builtins::logtest(&[left, right]),
        "LOGANDC1" => crate::builtins::logandc1(&[left, right]),
        "LOGANDC2" => crate::builtins::logandc2(&[left, right]),
        "LOGEQV" => crate::builtins::logeqv(&[left, right]),
        "LOGNAND" => crate::builtins::lognand(&[left, right]),
        "LOGNOR" => crate::builtins::lognor(&[left, right]),
        "LOGORC1" => crate::builtins::logorc1(&[left, right]),
        "LOGORC2" => crate::builtins::logorc2(&[left, right]),
        "LOGBITP" => crate::builtins::logbitp(&[left, right]),
        "EXPT" => crate::builtins::exponentiate(&[left, right]),
        _ => Err(invalid("unknown numeric binary operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_numeric_random_instruction(
    stack: &mut Vec<Value>,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("random has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    stack.push(crate::builtins::random(&arguments)?);
    Ok(())
}

pub fn execute_numeric_boole_instruction(
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    let right = stack
        .pop()
        .ok_or_else(|| invalid("BOOLE has too few stack values", span))?;
    let left = stack
        .pop()
        .ok_or_else(|| invalid("BOOLE has too few stack values", span))?;
    let operation = stack
        .pop()
        .ok_or_else(|| invalid("BOOLE has too few stack values", span))?;
    let result = crate::builtins::boole(&[operation, left, right])?;
    stack.push(result);
    Ok(())
}

pub fn execute_numeric_bitfield_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid(
            "numeric bitfield operation has too few stack values",
            span,
        ));
    }
    let start = stack.len() - argument_count;
    let arguments = stack.split_off(start);
    let result = match operation {
        "BYTE" => crate::builtins::byte(&arguments),
        "LDB" => crate::builtins::ldb(&arguments),
        "MASK-FIELD" => crate::builtins::mask_field(&arguments),
        "DPB" => crate::builtins::dpb(&arguments),
        "DEPOSIT-FIELD" => crate::builtins::deposit_field(&arguments),
        _ => Err(invalid("unknown numeric bitfield operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_numeric_float_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid(
            "numeric float operation has too few stack values",
            span,
        ));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    let result = match operation {
        "FLOAT" => crate::builtins::float_value(&arguments),
        "FLOAT-SIGN" => crate::builtins::float_sign(&arguments),
        "FLOAT-DIGITS" => crate::builtins::float_digits(&arguments),
        "FLOAT-PRECISION" => crate::builtins::float_precision(&arguments),
        "FLOAT-RADIX" => crate::builtins::float_radix(&arguments),
        "SCALE-FLOAT" => crate::builtins::scale_float(&arguments),
        "DECODE-FLOAT" => crate::builtins::decode_float(&arguments),
        "INTEGER-DECODE-FLOAT" => crate::builtins::integer_decode_float(&arguments),
        "LOG" => crate::builtins::logarithm(&arguments),
        "ATAN" => crate::builtins::arc_tangent(&arguments),
        "COMPLEX" => crate::builtins::complex(&arguments),
        _ => Err(invalid("unknown numeric float operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

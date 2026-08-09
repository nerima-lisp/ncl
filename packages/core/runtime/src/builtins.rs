use std::cmp::Ordering;
use std::cell::RefCell;
use std::rc::Rc;

use ncl_syntax::{ReadError, ReadErrorKind, Reader, Span};

use crate::environment::normalize_name;
use crate::evaluator::quoted_form_value;
use crate::package::{self, COMMON_LISP_PACKAGE, KEYWORD_PACKAGE};
use crate::{Environment, Function, Rational, RuntimeError, Stream, Value};

pub fn install(environment: &Environment) {
    for (name, function) in [
        ("+", add as _),
        ("-", subtract as _),
        ("*", multiply as _),
        ("/", divide as _),
        ("expt", exponentiate as _),
        ("sqrt", square_root as _),
        ("signum", signum as _),
        ("float", float_value as _),
        ("rational", rational as _),
        ("rationalize", rationalize as _),
        ("=", numeric_equal as _),
        ("<", less_than as _),
        (">", greater_than as _),
        ("<=", less_equal as _),
        (">=", greater_equal as _),
        ("zerop", zerop as _),
        ("plusp", plusp as _),
        ("minusp", minusp as _),
        ("evenp", evenp as _),
        ("oddp", oddp as _),
        ("min", minimum as _),
        ("max", maximum as _),
        ("abs", absolute as _),
        ("1+", increment as _),
        ("1-", decrement as _),
        ("floor", floor as _),
        ("ceiling", ceiling as _),
        ("truncate", truncate as _),
        ("round", round as _),
        ("gcd", greatest_common_divisor as _),
        ("lcm", least_common_multiple as _),
        ("numerator", numerator as _),
        ("denominator", denominator as _),
        ("mod", modulo as _),
        ("rem", remainder as _),
        ("ash", arithmetic_shift as _),
        ("logand", logand as _),
        ("logior", logior as _),
        ("logxor", logxor as _),
        ("lognot", lognot as _),
        ("logtest", logtest as _),
        ("logcount", logcount as _),
        ("integer-length", integer_length as _),
        ("parse-integer", parse_integer as _),
        ("list", list as _),
        ("list*", list_star as _),
        ("make-list", make_list as _),
        ("values-list", values_list as _),
        ("list-length", list_length as _),
        ("nthcdr", nthcdr as _),
        ("acons", acons as _),
        ("pairlis", pairlis as _),
        ("cons", cons as _),
        ("car", car as _),
        ("cdr", cdr as _),
        ("first", first as _),
        ("rest", rest as _),
        ("append", append as _),
        ("nconc", nconc as _),
        ("revappend", revappend as _),
        ("nreconc", nreconc as _),
        ("length", length as _),
        ("reverse", reverse as _),
        ("nreverse", nreverse as _),
        ("last", last as _),
        ("butlast", butlast as _),
        ("nbutlast", nbutlast as _),
        ("copy-list", copy_list as _),
        ("copy-alist", copy_alist as _),
        ("copy-tree", copy_tree as _),
        ("vector", vector as _),
        ("make-array", make_array as _),
        ("make-sequence", make_sequence as _),
        ("aref", aref as _),
        ("row-major-aref", row_major_aref as _),
        ("array-row-major-index", array_row_major_index as _),
        ("array-in-bounds-p", array_in_bounds_p as _),
        ("array-element-type", array_element_type as _),
        ("simple-array-p", simple_array_p as _),
        ("arrayp", arrayp as _),
        ("array-rank", array_rank as _),
        ("array-dimensions", array_dimensions as _),
        ("array-dimension", array_dimension as _),
        ("array-total-size", array_total_size as _),
        ("make-hash-table", make_hash_table as _),
        ("gethash", gethash as _),
        ("remhash", remhash as _),
        ("clrhash", clrhash as _),
        ("hash-table-p", hash_table_p as _),
        ("hash-table-count", hash_table_count as _),
        ("hash-table-test", hash_table_test_value as _),
        ("nth", nth as _),
        ("elt", elt as _),
        ("subseq", subseq as _),
        ("fill", fill as _),
        ("replace", replace as _),
        ("copy-seq", copy_seq as _),
        ("concatenate", concatenate as _),
        ("coerce", coerce as _),
        ("string", string_value as _),
        ("make-string", make_string as _),
        ("char", character as _),
        ("char-code", char_code as _),
        ("code-char", code_char as _),
        ("char=", character_equal as _),
        ("char-equal", character_case_equal as _),
        ("char<", character_less_than as _),
        ("char>", character_greater_than as _),
        ("char<=", character_less_equal as _),
        ("char>=", character_greater_equal as _),
        ("char-upcase", character_upcase as _),
        ("char-downcase", character_downcase as _),
        ("string=", string_equal as _),
        ("string-equal", string_case_equal as _),
        ("string<", string_less_than as _),
        ("string>", string_greater_than as _),
        ("string<=", string_less_equal as _),
        ("string>=", string_greater_equal as _),
        ("string-upcase", string_upcase as _),
        ("string-downcase", string_downcase as _),
        ("string-capitalize", string_capitalize as _),
        ("nstring-upcase", nstring_upcase as _),
        ("nstring-downcase", nstring_downcase as _),
        ("nstring-capitalize", nstring_capitalize as _),
        ("string-trim", string_trim as _),
        ("string-left-trim", string_left_trim as _),
        ("string-right-trim", string_right_trim as _),
        ("getf", getf as _),
        ("get-properties", get_properties as _),
        ("null", null as _),
        ("not", null as _),
        ("endp", endp as _),
        ("atom", atom as _),
        ("consp", consp as _),
        ("listp", listp as _),
        ("numberp", numberp as _),
        ("integerp", integerp as _),
        ("floatp", floatp as _),
        ("rationalp", rationalp as _),
        ("stringp", stringp as _),
        ("simple-string-p", simple_string_p as _),
        ("characterp", characterp as _),
        ("symbolp", symbolp as _),
        ("packagep", packagep as _),
        ("keywordp", keywordp as _),
        ("symbol-name", symbol_name_value as _),
        ("symbol-package", symbol_package_value as _),
        ("vectorp", vectorp as _),
        ("simple-vector-p", simple_vector_p as _),
        ("functionp", functionp as _),
        ("eq", eq as _),
        ("eql", eql as _),
        ("equal", equal as _),
        ("equalp", equalp as _),
        ("identity", identity as _),
        ("type-of", type_of as _),
        ("typep", typep as _),
        ("__NCL_THE_CHECK", the_check as _),
        ("__NCL_ECASE_ERROR", ecase_error as _),
        ("__NCL_ETYPECASE_ERROR", etypecase_error as _),
        ("print", print_value as _),
        ("princ", princ as _),
        ("prin1", prin1 as _),
        ("format", format_value as _),
        ("write-to-string", write_to_string as _),
        ("read-from-string", read_from_string as _),
        ("make-string-input-stream", make_string_input_stream as _),
        ("make-string-output-stream", make_string_output_stream as _),
        ("get-output-stream-string", get_output_stream_string as _),
        ("read-char", read_char as _),
        ("peek-char", peek_char as _),
        ("unread-char", unread_char as _),
        ("read-line", read_line as _),
        ("write-char", write_char as _),
        ("write-string", write_string as _),
        ("terpri", terpri as _),
        ("fresh-line", fresh_line as _),
        ("write-line", write_line as _),
        ("close", close_stream as _),
        ("streamp", streamp as _),
        ("input-stream-p", input_stream_p as _),
        ("output-stream-p", output_stream_p as _),
    ] {
        let value = Value::builtin(name, function);
        let normalized = normalize_name(name);
        environment.define(normalized.clone(), value.clone());
        environment.define(format!("{COMMON_LISP_PACKAGE}::{normalized}"), value);
    }
    for name in [
        "EVAL",
        "MAP",
        "REDUCE",
        "MAP-INTO",
        "FIND",
        "POSITION",
        "COUNT",
        "SEARCH",
        "MISMATCH",
        "SORT",
        "STABLE-SORT",
        "MERGE",
        "EVERY",
        "SOME",
        "NOTANY",
        "NOTEVERY",
        "REMOVE",
        "REMOVE-IF",
        "REMOVE-IF-NOT",
        "DELETE",
        "DELETE-IF",
        "DELETE-IF-NOT",
        "REMOVE-DUPLICATES",
        "DELETE-DUPLICATES",
        "SUBSTITUTE",
        "SUBSTITUTE-IF",
        "SUBSTITUTE-IF-NOT",
        "NSUBSTITUTE",
        "NSUBSTITUTE-IF",
        "NSUBSTITUTE-IF-NOT",
        "UNION",
        "NUNION",
        "INTERSECTION",
        "NINTERSECTION",
        "SET-DIFFERENCE",
        "NSET-DIFFERENCE",
        "SET-EXCLUSIVE-OR",
        "NSET-EXCLUSIVE-OR",
        "SUBSETP",
        "MEMBER",
        "MEMBER-IF",
        "MEMBER-IF-NOT",
        "ADJOIN",
        "ASSOC",
        "ASSOC-IF",
        "ASSOC-IF-NOT",
        "RASSOC",
        "RASSOC-IF",
        "RASSOC-IF-NOT",
        "MAPCAR",
        "MAPC",
        "MAPL",
        "MAPLIST",
        "MAPCAN",
        "MAPCON",
        "MAKE-SYMBOL",
        "GENSYM",
        "INTERN",
        "FIND-SYMBOL",
        "FIND-PACKAGE",
        "PACKAGE-NAME",
        "PACKAGE-USE-LIST",
        "LIST-ALL-PACKAGES",
        "USE-PACKAGE",
        "UNUSE-PACKAGE",
        "EXPORT",
        "UNEXPORT",
        "IMPORT",
        "UNINTERN",
        "SHADOW",
        "SHADOWING-IMPORT",
        "BOUNDP",
        "CONSTANTP",
        "FBOUNDP",
        "FDEFINITION",
        "SYMBOL-FUNCTION",
        "SYMBOL-VALUE",
        "GET",
        "PUTPROP",
        "REMPROP",
        "SYMBOL-PLIST",
        "SET",
        "MAKUNBOUND",
        "FMAKUNBOUND",
        "MAKE-INSTANCE",
        "SLOT-VALUE",
        "CLASS-OF",
        "FIND-CLASS",
        "CLASS-NAME",
        "SLOT-EXISTS-P",
        "SLOT-BOUNDP",
        "SLOT-MAKUNBOUND",
        "CALL-NEXT-METHOD",
        "NEXT-METHOD-P",
        "INVOKE-RESTART",
    ] {
        let value = Value::primitive(name);
        environment.define(name, value.clone());
        environment.define(format!("{COMMON_LISP_PACKAGE}::{name}"), value);
    }
    for (name, value) in [("NIL", Value::Nil), ("T", Value::boolean(true))] {
        environment.define(name, value.clone());
        environment.define(format!("{COMMON_LISP_PACKAGE}::{name}"), value);
    }
}

fn add(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = Number::Integer(0);
    for argument in arguments {
        let value = number_argument("+", argument)?;
        result = if result.is_float() || value.is_float() {
            Number::Float(result.as_float() + value.as_float())
        } else {
            exact_binary(result, value, '+')?
        };
    }
    number_to_value(result)
}

fn subtract(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("-", "at least one", 0));
    }
    let values = arguments
        .iter()
        .map(|value| number_argument("-", value))
        .collect::<Result<Vec<_>, _>>()?;
    let mut result = values[0];
    if values.len() == 1 {
        result = negate_number(result)?;
    } else {
        for value in &values[1..] {
            result = if result.is_float() || value.is_float() {
                Number::Float(result.as_float() - value.as_float())
            } else {
                exact_binary(result, *value, '-')?
            };
        }
    }
    number_to_value(result)
}

fn multiply(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = Number::Integer(1);
    for argument in arguments {
        let value = number_argument("*", argument)?;
        result = if result.is_float() || value.is_float() {
            Number::Float(result.as_float() * value.as_float())
        } else {
            exact_binary(result, value, '*')?
        };
    }
    number_to_value(result)
}

fn divide(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("/", "at least one", 0));
    }
    let values = arguments
        .iter()
        .map(|value| number_argument("/", value))
        .collect::<Result<Vec<_>, _>>()?;
    let mut result;
    if values.len() == 1 {
        result = if values[0].is_float() {
            let divisor = values[0].as_float();
            if divisor == 0.0 {
                return Err(RuntimeError::DivisionByZero);
            }
            Number::Float(1.0 / divisor)
        } else {
            exact_binary(Number::Integer(1), values[0], '/')?
        };
    } else {
        result = values[0];
        for value in &values[1..] {
            result = if result.is_float() || value.is_float() {
                let divisor = value.as_float();
                if divisor == 0.0 {
                    return Err(RuntimeError::DivisionByZero);
                }
                Number::Float(result.as_float() / divisor)
            } else {
                exact_binary(result, *value, '/')?
            };
        }
    }
    number_to_value(result)
}

fn exponentiate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "expt", 2)?;
    let base = number_argument("expt", &arguments[0])?;
    let exponent = number_argument("expt", &arguments[1])?;

    if !base.is_float() {
        if let Some((exponent_numerator, exponent_denominator)) = exponent.exact_parts() {
            if exponent_denominator == 1 {
                return number_to_value(exact_power(base, exponent_numerator)?);
            }
        }
    }

    Ok(Value::Float(base.as_float().powf(exponent.as_float())))
}

fn square_root(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "sqrt", 1)?;
    match number_argument("sqrt", &arguments[0])? {
        Number::Integer(value) if value >= 0 => {
            let root = integer_square_root(value as u128);
            if root * root == value as u128 {
                Ok(Value::Integer(root as i64))
            } else {
                Ok(Value::Float((value as f64).sqrt()))
            }
        }
        Number::Integer(_) => Err(negative_real_error("sqrt")),
        Number::Rational(value) if value.numerator() >= 0 => {
            let numerator = value.numerator() as u128;
            let denominator = value.denominator() as u128;
            let numerator_root = integer_square_root(numerator);
            let denominator_root = integer_square_root(denominator);
            if numerator_root * numerator_root == numerator
                && denominator_root * denominator_root == denominator
            {
                rational_number(numerator_root as i128, denominator_root as i128)
                    .and_then(number_to_value)
            } else {
                Ok(Value::Float(
                    (value.numerator() as f64 / value.denominator() as f64).sqrt(),
                ))
            }
        }
        Number::Rational(_) => Err(negative_real_error("sqrt")),
        Number::Float(value) if value >= 0.0 => Ok(Value::Float(value.sqrt())),
        Number::Float(_) => Err(negative_real_error("sqrt")),
    }
}

fn integer_square_root(value: u128) -> u128 {
    if value < 2 {
        return value;
    }
    let bits = 128 - value.leading_zeros();
    let mut root = 1u128 << (bits / 2 + 1);
    loop {
        let next = (root + value / root) / 2;
        if next >= root {
            return root;
        }
        root = next;
    }
}

fn negative_real_error(function: &str) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: format!("{function} of a negative real requires complex numbers"),
        span: None,
    }
}

fn signum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "signum", 1)?;
    match number_argument("signum", &arguments[0])? {
        Number::Integer(value) => Ok(Value::Integer(value.signum())),
        Number::Rational(value) => Ok(Value::Integer(value.numerator().signum())),
        Number::Float(value) if value.is_nan() => Err(RuntimeError::InvalidForm {
            message: "signum of NaN is undefined".to_owned(),
            span: None,
        }),
        Number::Float(value) if value == 0.0 => Ok(Value::Float(value)),
        Number::Float(value) => Ok(Value::Float(if value.is_sign_negative() {
            -1.0
        } else {
            1.0
        })),
    }
}

fn float_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() || arguments.len() > 2 {
        return Err(arity("float", "1 to 2", arguments.len()));
    }
    let number = number_argument("float", &arguments[0])?;
    if let Some(prototype) = arguments.get(1) {
        if !matches!(prototype, Value::Float(_)) {
            return Err(type_error("float", "a float prototype", prototype));
        }
    }
    Ok(Value::Float(number.as_float()))
}

fn rational(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rational", 1)?;
    match number_argument("rational", &arguments[0])? {
        Number::Integer(value) => Ok(Value::Integer(value)),
        Number::Rational(value) => Value::rational(
            i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => rational_from_float(value),
    }
}

fn rational_from_float(value: f64) -> Result<Value, RuntimeError> {
    if !value.is_finite() {
        return Err(RuntimeError::InvalidForm {
            message: "rational requires a finite real".to_owned(),
            span: None,
        });
    }
    if value == 0.0 {
        return Ok(Value::Integer(0));
    }

    const FRACTION_MASK: u64 = (1 << 52) - 1;
    let bits = value.to_bits();
    let negative = bits >> 63 != 0;
    let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
    let mut significand = bits & FRACTION_MASK;
    let mut exponent = if exponent_bits == 0 {
        -1074
    } else {
        significand |= 1 << 52;
        exponent_bits - 1023 - 52
    };

    if exponent < 0 {
        let canceled = significand
            .trailing_zeros()
            .min((-exponent) as u32);
        significand >>= canceled;
        exponent += canceled as i32;
    }

    let mut numerator = i128::from(significand);
    if negative {
        numerator = -numerator;
    }
    let denominator = if exponent >= 0 {
        numerator = numerator
            .checked_shl(exponent as u32)
            .ok_or(RuntimeError::NumericOverflow)?;
        1
    } else {
        1i128
            .checked_shl((-exponent) as u32)
            .ok_or(RuntimeError::NumericOverflow)?
    };
    Value::rational(numerator, denominator)
}

fn rationalize(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rationalize", 1)?;
    match number_argument("rationalize", &arguments[0])? {
        Number::Integer(value) => Ok(Value::Integer(value)),
        Number::Rational(value) => Value::rational(
            i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => rationalize_float(value),
    }
}

fn rationalize_float(value: f64) -> Result<Value, RuntimeError> {
    if !value.is_finite() {
        return Err(RuntimeError::InvalidForm {
            message: "rationalize requires a finite real".to_owned(),
            span: None,
        });
    }
    if value == 0.0 {
        return Ok(Value::Integer(0));
    }

    let tolerance = (value.abs() * f64::EPSILON / 2.0).max(f64::MIN_POSITIVE);
    let (numerator, denominator) = simplest_rational(value - tolerance, value + tolerance)?;
    number_to_value(rational_number(numerator, denominator)?)
}

fn simplest_rational(lower: f64, upper: f64) -> Result<(i128, i128), RuntimeError> {
    if !lower.is_finite() || !upper.is_finite() || lower > upper {
        return Err(RuntimeError::NumericOverflow);
    }
    if lower <= 0.0 && upper >= 0.0 {
        return Ok((0, 1));
    }
    if upper < 0.0 {
        let (numerator, denominator) = simplest_positive_rational(-upper, -lower, 0)?;
        return Ok((-numerator, denominator));
    }
    simplest_positive_rational(lower, upper, 0)
}

fn simplest_positive_rational(
    lower: f64,
    upper: f64,
    depth: u32,
) -> Result<(i128, i128), RuntimeError> {
    if depth > 128 || !lower.is_finite() || !upper.is_finite() || lower <= 0.0 || lower > upper {
        return Err(RuntimeError::NumericOverflow);
    }

    let lower_floor = lower.floor();
    let upper_floor = upper.floor();
    if lower == lower_floor {
        return Ok((lower_floor as i128, 1));
    }
    if lower_floor < upper_floor {
        return Ok(((lower_floor as i128) + 1, 1));
    }

    let lower_fraction = lower - lower_floor;
    let upper_fraction = upper - lower_floor;
    let (reciprocal_numerator, reciprocal_denominator) = simplest_positive_rational(
        1.0 / upper_fraction,
        1.0 / lower_fraction,
        depth + 1,
    )?;
    let numerator = (lower_floor as i128)
        .checked_mul(reciprocal_numerator)
        .and_then(|value| value.checked_add(reciprocal_denominator))
        .ok_or(RuntimeError::NumericOverflow)?;
    Ok((numerator, reciprocal_numerator))
}

fn exact_power(base: Number, exponent: i64) -> Result<Number, RuntimeError> {
    let (mut numerator, mut denominator) = base
        .exact_parts()
        .expect("exact power received a float");
    let negative_exponent = exponent < 0;
    if negative_exponent && numerator == 0 {
        return Err(RuntimeError::DivisionByZero);
    }
    if negative_exponent {
        std::mem::swap(&mut numerator, &mut denominator);
    }

    let magnitude = exponent.unsigned_abs();
    rational_number(
        checked_power(i128::from(numerator), magnitude)?,
        checked_power(i128::from(denominator), magnitude)?,
    )
}

fn checked_power(base: i128, mut exponent: u64) -> Result<i128, RuntimeError> {
    let mut result = 1i128;
    let mut factor = base;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = result
                .checked_mul(factor)
                .ok_or(RuntimeError::NumericOverflow)?;
        }
        exponent >>= 1;
        if exponent != 0 {
            factor = factor
                .checked_mul(factor)
                .ok_or(RuntimeError::NumericOverflow)?;
        }
    }
    Ok(result)
}

fn numeric_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers("=", arguments, |ordering| ordering == Ordering::Equal)
}

fn less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers("<", arguments, |ordering| ordering == Ordering::Less)
}

fn greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers(">", arguments, |ordering| ordering == Ordering::Greater)
}

fn less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers("<=", arguments, |ordering| ordering != Ordering::Greater)
}

fn greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers(">=", arguments, |ordering| ordering != Ordering::Less)
}

fn zerop(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "zerop", 1)?;
    Ok(Value::boolean(
        number_argument("zerop", &arguments[0])?.as_float() == 0.0,
    ))
}

fn plusp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "plusp", 1)?;
    Ok(Value::boolean(
        number_argument("plusp", &arguments[0])?.as_float() > 0.0,
    ))
}

fn minusp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "minusp", 1)?;
    Ok(Value::boolean(
        number_argument("minusp", &arguments[0])?.as_float() < 0.0,
    ))
}

fn evenp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "evenp", 1)?;
    Ok(Value::boolean(
        integer_argument("evenp", &arguments[0])? % 2 == 0,
    ))
}

fn oddp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "oddp", 1)?;
    Ok(Value::boolean(
        integer_argument("oddp", &arguments[0])? % 2 != 0,
    ))
}

fn minimum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    extreme(arguments, "min", true)
}

fn maximum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    extreme(arguments, "max", false)
}

fn extreme(
    arguments: &[Value],
    function: &str,
    choose_minimum: bool,
) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity(function, "at least one", 0));
    }
    let values = arguments
        .iter()
        .map(|value| number_argument(function, value))
        .collect::<Result<Vec<_>, _>>()?;
    let mut result = values[0];
    for value in &values[1..] {
        let ordering = compare_number_values(*value, result);
        if (choose_minimum && ordering == Ordering::Less)
            || (!choose_minimum && ordering == Ordering::Greater)
        {
            result = *value;
        }
    }
    number_to_value(result)
}

fn absolute(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "abs", 1)?;
    match number_argument("abs", &arguments[0])? {
        Number::Integer(value) => value
            .checked_abs()
            .map(Value::Integer)
            .ok_or(RuntimeError::NumericOverflow),
        Number::Rational(value) => number_to_value(rational_number(
            i128::from(value.numerator()).abs(),
            i128::from(value.denominator()),
        )?),
        Number::Float(value) => Ok(Value::Float(value.abs())),
    }
}

fn compare_numbers(
    function: &str,
    arguments: &[Value],
    comparison: fn(Ordering) -> bool,
) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity(function, "at least one", 0));
    }
    let values = arguments
        .iter()
        .map(|value| number_argument(function, value))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::boolean(values.windows(2).all(|window| {
        comparison(compare_number_values(window[0], window[1]))
    })))
}

fn increment(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "1+", 1)?;
    add(&[arguments[0].clone(), Value::Integer(1)])
}

fn decrement(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "1-", 1)?;
    subtract(&[arguments[0].clone(), Value::Integer(1)])
}

#[derive(Clone, Copy)]
enum RoundingMode {
    Floor,
    Ceiling,
    Truncate,
    Round,
}

fn floor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "floor", RoundingMode::Floor)
}

fn ceiling(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "ceiling", RoundingMode::Ceiling)
}

fn truncate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "truncate", RoundingMode::Truncate)
}

fn round(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "round", RoundingMode::Round)
}

fn quotient_and_remainder(
    arguments: &[Value],
    function: &str,
    mode: RoundingMode,
) -> Result<Value, RuntimeError> {
    if arguments.len() != 1 && arguments.len() != 2 {
        return Err(arity(function, "one or two", arguments.len()));
    }
    let dividend = number_argument(function, &arguments[0])?;
    let divisor = if arguments.len() == 2 {
        number_argument(function, &arguments[1])?
    } else {
        Number::Integer(1)
    };
    if dividend.is_float() || divisor.is_float() {
        float_quotient_and_remainder(dividend, divisor, mode)
    } else {
        exact_quotient_and_remainder(dividend, divisor, mode)
    }
}

fn exact_quotient_and_remainder(
    dividend: Number,
    divisor: Number,
    mode: RoundingMode,
) -> Result<Value, RuntimeError> {
    let (dividend_numerator, dividend_denominator) = dividend
        .exact_parts()
        .expect("exact quotient received a float");
    let (divisor_numerator, divisor_denominator) = divisor
        .exact_parts()
        .expect("exact quotient received a float");
    if divisor_numerator == 0 {
        return Err(RuntimeError::DivisionByZero);
    }

    let dividend_numerator = i128::from(dividend_numerator);
    let dividend_denominator = i128::from(dividend_denominator);
    let divisor_numerator = i128::from(divisor_numerator);
    let divisor_denominator = i128::from(divisor_denominator);
    let mut quotient_numerator = dividend_numerator * divisor_denominator;
    let mut quotient_denominator = dividend_denominator * divisor_numerator;
    if quotient_denominator < 0 {
        quotient_numerator = -quotient_numerator;
        quotient_denominator = -quotient_denominator;
    }
    let truncated = quotient_numerator / quotient_denominator;
    let quotient = adjust_exact_quotient(
        truncated,
        quotient_numerator,
        quotient_denominator,
        mode,
    )?;
    let quotient = i64::try_from(quotient).map_err(|_| RuntimeError::NumericOverflow)?;
    let remainder = rational_number(
        dividend_numerator * divisor_denominator
            - i128::from(quotient) * divisor_numerator * dividend_denominator,
        dividend_denominator * divisor_denominator,
    )?;
    Ok(Value::values(vec![
        Value::Integer(quotient),
        number_to_value(remainder)?,
    ]))
}

fn adjust_exact_quotient(
    truncated: i128,
    numerator: i128,
    denominator: i128,
    mode: RoundingMode,
) -> Result<i128, RuntimeError> {
    let remainder = numerator % denominator;
    if remainder == 0 {
        return Ok(truncated);
    }
    let direction = if numerator < 0 { -1 } else { 1 };
    match mode {
        RoundingMode::Truncate => Ok(truncated),
        RoundingMode::Floor if direction < 0 => truncated
            .checked_sub(1)
            .ok_or(RuntimeError::NumericOverflow),
        RoundingMode::Ceiling if direction > 0 => truncated
            .checked_add(1)
            .ok_or(RuntimeError::NumericOverflow),
        RoundingMode::Round => {
            let distance = remainder.abs() * 2;
            if distance > denominator || (distance == denominator && truncated % 2 != 0) {
                truncated
                    .checked_add(direction)
                    .ok_or(RuntimeError::NumericOverflow)
            } else {
                Ok(truncated)
            }
        }
        _ => Ok(truncated),
    }
}

fn float_quotient_and_remainder(
    dividend: Number,
    divisor: Number,
    mode: RoundingMode,
) -> Result<Value, RuntimeError> {
    let dividend = dividend.as_float();
    let divisor = divisor.as_float();
    if divisor == 0.0 {
        return Err(RuntimeError::DivisionByZero);
    }
    let ratio = dividend / divisor;
    let rounded = match mode {
        RoundingMode::Floor => ratio.floor(),
        RoundingMode::Ceiling => ratio.ceil(),
        RoundingMode::Truncate => ratio.trunc(),
        RoundingMode::Round => round_float(ratio),
    };
    let quotient = float_integer(rounded)?;
    let remainder = Value::Float(dividend - quotient as f64 * divisor);
    Ok(Value::values(vec![Value::Integer(quotient), remainder]))
}

fn round_float(value: f64) -> f64 {
    let truncated = value.trunc();
    let fraction = (value - truncated).abs();
    if fraction > 0.5 || (fraction == 0.5 && truncated % 2.0 != 0.0) {
        truncated + value.signum()
    } else {
        truncated
    }
}

fn float_integer(value: f64) -> Result<i64, RuntimeError> {
    if !value.is_finite() || value < i64::MIN as f64 || value >= 9_223_372_036_854_775_808.0 {
        return Err(RuntimeError::NumericOverflow);
    }
    Ok(value as i64)
}

fn greatest_common_divisor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = 0i128;
    for argument in arguments {
        result = integer_gcd(result, i128::from(integer_argument("gcd", argument)?));
    }
    i64::try_from(result)
        .map(Value::Integer)
        .map_err(|_| RuntimeError::NumericOverflow)
}

fn least_common_multiple(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = 1i128;
    for argument in arguments {
        let value = i128::from(integer_argument("lcm", argument)?);
        if result == 0 || value == 0 {
            result = 0;
            continue;
        }
        let divisor = integer_gcd(result, value);
        result = (result / divisor)
            .checked_mul(value.abs())
            .ok_or(RuntimeError::NumericOverflow)?;
    }
    i64::try_from(result)
        .map(Value::Integer)
        .map_err(|_| RuntimeError::NumericOverflow)
}

fn integer_gcd(mut left: i128, mut right: i128) -> i128 {
    left = left.abs();
    right = right.abs();
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn numerator(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "numerator", 1)?;
    match arguments[0] {
        Value::Integer(value) => Ok(Value::Integer(value)),
        Value::Rational(value) => Ok(Value::Integer(value.numerator())),
        ref value => Err(type_error("numerator", "rational", value)),
    }
}

fn denominator(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "denominator", 1)?;
    match arguments[0] {
        Value::Integer(_) => Ok(Value::Integer(1)),
        Value::Rational(value) => Ok(Value::Integer(value.denominator())),
        ref value => Err(type_error("denominator", "rational", value)),
    }
}

fn modulo(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "mod", 2)?;
    let left = integer_argument("mod", &arguments[0])?;
    let right = integer_argument("mod", &arguments[1])?;
    let remainder = integer_remainder(left, right)?;
    if remainder != 0 && (left < 0) != (right < 0) {
        remainder
            .checked_add(right)
            .map(Value::Integer)
            .ok_or(RuntimeError::NumericOverflow)
    } else {
        Ok(Value::Integer(remainder))
    }
}

fn remainder(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rem", 2)?;
    let left = integer_argument("rem", &arguments[0])?;
    let right = integer_argument("rem", &arguments[1])?;
    integer_remainder(left, right).map(Value::Integer)
}

fn integer_remainder(left: i64, right: i64) -> Result<i64, RuntimeError> {
    if right == 0 {
        return Err(RuntimeError::DivisionByZero);
    }
    if left == i64::MIN && right == -1 {
        return Ok(0);
    }
    left.checked_rem(right).ok_or(RuntimeError::NumericOverflow)
}

fn arithmetic_shift(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "ash", 2)?;
    let value = integer_argument("ash", &arguments[0])?;
    let count = integer_argument("ash", &arguments[1])?;
    if count >= 0 {
        if count >= 64 {
            return if value == 0 {
                Ok(Value::Integer(0))
            } else {
                Err(RuntimeError::NumericOverflow)
            };
        }
        return value
            .checked_shl(count as u32)
            .map(Value::Integer)
            .ok_or(RuntimeError::NumericOverflow);
    }

    let shift = if count == i64::MIN {
        u64::MAX
    } else {
        (-count) as u64
    };
    Ok(Value::Integer(if shift >= 64 {
        if value < 0 {
            -1
        } else {
            0
        }
    } else {
        value >> shift as u32
    }))
}

fn logand(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logand", -1, |left, right| left & right)
}

fn logior(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logior", 0, |left, right| left | right)
}

fn logxor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logxor", 0, |left, right| left ^ right)
}

fn bitwise(
    arguments: &[Value],
    function: &str,
    identity: i64,
    operation: fn(i64, i64) -> i64,
) -> Result<Value, RuntimeError> {
    let mut result = identity;
    for argument in arguments {
        result = operation(result, integer_argument(function, argument)?);
    }
    Ok(Value::Integer(result))
}

fn lognot(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "lognot", 1)?;
    Ok(Value::Integer(!integer_argument("lognot", &arguments[0])?))
}

fn logtest(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logtest", 2)?;
    let left = integer_argument("logtest", &arguments[0])?;
    let right = integer_argument("logtest", &arguments[1])?;
    Ok(Value::boolean((left & right) != 0))
}

fn logcount(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logcount", 1)?;
    let value = integer_argument("logcount", &arguments[0])?;
    let count = if value < 0 {
        (!value).count_ones()
    } else {
        value.count_ones()
    };
    Ok(Value::Integer(count as i64))
}

fn integer_length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "integer-length", 1)?;
    let value = integer_argument("integer-length", &arguments[0])?;
    let magnitude = if value < 0 { !value } else { value } as u64;
    Ok(Value::Integer((64 - magnitude.leading_zeros()) as i64))
}

fn parse_integer(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() || (arguments.len() - 1) % 2 != 0 {
        return Err(arity(
            "parse-integer",
            "a string and keyword/value pairs",
            arguments.len(),
        ));
    }
    let chars = match &arguments[0] {
        Value::String(value) => value.as_ref().chars().collect::<Vec<_>>(),
        value => return Err(type_error("parse-integer", "a string", value)),
    };
    let mut start = 0;
    let mut end = chars.len();
    let mut radix = 10_i64;
    let mut junk_allowed = false;
    for pair in arguments[1..].chunks_exact(2) {
        match array_option_name("parse-integer", &pair[0])?.as_str() {
            "START" => start = index_argument("parse-integer", &pair[1])?,
            "END" => end = index_argument("parse-integer", &pair[1])?,
            "RADIX" => radix = integer_argument("parse-integer", &pair[1])?,
            "JUNK-ALLOWED" => junk_allowed = pair[1].is_truthy(),
            option => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("parse-integer does not accept :{option}"),
                    span: None,
                });
            }
        }
    }
    if start > end || end > chars.len() {
        return Err(RuntimeError::InvalidForm {
            message: "parse-integer bounds are invalid".to_string(),
            span: None,
        });
    }
    if !(2..=36).contains(&radix) {
        return Err(RuntimeError::InvalidForm {
            message: format!("parse-integer radix must be between 2 and 36, got {radix}"),
            span: None,
        });
    }
    let radix = u32::try_from(radix).expect("parse-integer radix was checked");
    let mut cursor = start;
    while cursor < end && chars[cursor].is_whitespace() {
        cursor += 1;
    }
    let negative = match chars.get(cursor) {
        Some('+') => {
            cursor += 1;
            false
        }
        Some('-') => {
            cursor += 1;
            true
        }
        _ => false,
    };
    let digits_start = cursor;
    let mut magnitude = 0_i128;
    while cursor < end {
        let Some(digit) = parse_integer_digit(chars[cursor]) else {
            break;
        };
        if digit >= radix {
            break;
        }
        magnitude = magnitude
            .checked_mul(i128::from(radix))
            .and_then(|value| value.checked_add(i128::from(digit)))
            .ok_or(RuntimeError::NumericOverflow)?;
        cursor += 1;
    }
    if cursor == digits_start {
        if junk_allowed {
            let position = i64::try_from(cursor).map_err(|_| RuntimeError::NumericOverflow)?;
            return Ok(Value::values(vec![Value::Nil, Value::Integer(position)]));
        }
        return Err(RuntimeError::InvalidForm {
            message: "parse-integer found no integer".to_string(),
            span: None,
        });
    }
    let signed = if negative {
        magnitude.checked_neg().ok_or(RuntimeError::NumericOverflow)?
    } else {
        magnitude
    };
    let integer = i64::try_from(signed).map_err(|_| RuntimeError::NumericOverflow)?;
    if junk_allowed {
        let position = i64::try_from(cursor).map_err(|_| RuntimeError::NumericOverflow)?;
        return Ok(Value::values(vec![Value::Integer(integer), Value::Integer(position)]));
    }
    let mut trailing = cursor;
    while trailing < end && chars[trailing].is_whitespace() {
        trailing += 1;
    }
    if trailing != end {
        return Err(RuntimeError::InvalidForm {
            message: "parse-integer found junk after the integer".to_string(),
            span: None,
        });
    }
    let position = i64::try_from(end).map_err(|_| RuntimeError::NumericOverflow)?;
    Ok(Value::values(vec![Value::Integer(integer), Value::Integer(position)]))
}

fn parse_integer_digit(character: char) -> Option<u32> {
    match character {
        '0'..='9' => Some(u32::from(character as u8 - b'0')),
        'A'..='Z' => Some(u32::from(character as u8 - b'A') + 10),
        'a'..='z' => Some(u32::from(character as u8 - b'a') + 10),
        _ => None,
    }
}

fn list(arguments: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::list(arguments.to_vec()))
}

fn list_star(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("list*", "at least one", 0));
    }
    if arguments.len() == 1 {
        return Ok(arguments[0].clone());
    }

    let mut values = arguments[..arguments.len() - 1].to_vec();
    match arguments.last().expect("arguments is non-empty") {
        Value::Nil | Value::List(_) => {
            let Some(items) = arguments.last().and_then(Value::list_items) else {
                unreachable!();
            };
            values.extend(items);
            Ok(Value::list(values))
        }
        Value::DottedList { items, tail } => {
            values.extend(items.iter().cloned());
            Ok(Value::dotted_list(values, tail.as_ref().clone()))
        }
        tail => Ok(Value::dotted_list(values, tail.clone())),
    }
}

fn make_list(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("make-list", "at least one", 0));
    }
    if (arguments.len() - 1) % 2 != 0 {
        return Err(arity(
            "make-list",
            "a size and keyword/value pairs",
            arguments.len(),
        ));
    }

    let size = index_argument("make-list", &arguments[0])?;
    let mut initial_element = Value::Nil;
    for pair in arguments[1..].chunks_exact(2) {
        match array_option_name("make-list", &pair[0])?.as_str() {
            "INITIAL-ELEMENT" => initial_element = pair[1].clone(),
            option => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("make-list does not accept :{option}"),
                    span: None,
                });
            }
        }
    }
    Ok(Value::list(vec![initial_element; size]))
}

fn values_list(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "values-list", 1)?;
    let Some(values) = arguments[0].list_items() else {
        return Err(type_error("values-list", "list", &arguments[0]));
    };
    Ok(Value::values(values))
}

fn list_length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "list-length", 1)?;
    let length = match &arguments[0] {
        Value::Nil => 0,
        Value::List(items) => items.len(),
        value => return Err(type_error("list-length", "proper list", value)),
    };
    Ok(Value::Integer(length as i64))
}

fn nthcdr(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "nthcdr", 2)?;
    let index = index_argument("nthcdr", &arguments[0])?;
    match &arguments[1] {
        Value::Nil => Ok(Value::Nil),
        Value::List(items) => Ok(Value::list(items.iter().skip(index).cloned().collect())),
        Value::DottedList { items, tail } if index < items.len() => Ok(Value::dotted_list(
            items.iter().skip(index).cloned().collect(),
            tail.as_ref().clone(),
        )),
        Value::DottedList { items, tail } if index == items.len() => Ok(tail.as_ref().clone()),
        value @ Value::DottedList { .. } => Err(type_error("nthcdr", "proper list", value)),
        value => Err(type_error("nthcdr", "list", value)),
    }
}

fn acons(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "acons", 3)?;
    let Some(alist) = arguments[2].list_items() else {
        return Err(type_error("acons", "list", &arguments[2]));
    };
    let mut result = Vec::with_capacity(alist.len() + 1);
    result.push(Value::dotted_list(
        vec![arguments[0].clone()],
        arguments[1].clone(),
    ));
    result.extend(alist);
    Ok(Value::list(result))
}

fn pairlis(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&arguments.len()) {
        return Err(arity("pairlis", "2 or 3", arguments.len()));
    }
    let Some(keys) = arguments[0].list_items() else {
        return Err(type_error("pairlis", "list", &arguments[0]));
    };
    let Some(values) = arguments[1].list_items() else {
        return Err(type_error("pairlis", "list", &arguments[1]));
    };
    if keys.len() != values.len() {
        return Err(RuntimeError::InvalidForm {
            message: "pairlis requires lists of equal length".to_string(),
            span: None,
        });
    }
    let mut result = match arguments.get(2) {
        Some(alist) => alist
            .list_items()
            .ok_or_else(|| type_error("pairlis", "list", alist))?,
        None => Vec::new(),
    };
    for (key, value) in keys.into_iter().zip(values) {
        result.insert(0, Value::dotted_list(vec![key], value));
    }
    Ok(Value::list(result))
}

fn cons(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "cons", 2)?;
    match &arguments[1] {
        Value::Nil => Ok(Value::list(vec![arguments[0].clone()])),
        Value::List(items) => {
            let mut values = Vec::with_capacity(items.len() + 1);
            values.push(arguments[0].clone());
            values.extend(items.iter().cloned());
            Ok(Value::list(values))
        }
        Value::DottedList { items, tail } => {
            let mut values = Vec::with_capacity(items.len() + 1);
            values.push(arguments[0].clone());
            values.extend(items.iter().cloned());
            Ok(Value::dotted_list(values, tail.as_ref().clone()))
        }
        _ => Ok(Value::dotted_list(
            vec![arguments[0].clone()],
            arguments[1].clone(),
        )),
    }
}

fn car(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "car", 1)?;
    match &arguments[0] {
        Value::Nil => Ok(Value::Nil),
        Value::List(items) => items
            .first()
            .cloned()
            .ok_or_else(|| type_error("car", "non-empty list", &arguments[0])),
        Value::DottedList { items, .. } => items
            .first()
            .cloned()
            .ok_or_else(|| type_error("car", "non-empty list", &arguments[0])),
        value => Err(type_error("car", "list", value)),
    }
}

fn cdr(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "cdr", 1)?;
    match &arguments[0] {
        Value::Nil => Ok(Value::Nil),
        Value::List(items) => Ok(Value::list(items.iter().skip(1).cloned().collect())),
        Value::DottedList { items, tail } if items.len() > 1 => Ok(Value::dotted_list(
            items.iter().skip(1).cloned().collect(),
            tail.as_ref().clone(),
        )),
        Value::DottedList { tail, .. } => Ok(tail.as_ref().clone()),
        value => Err(type_error("cdr", "list", value)),
    }
}

fn first(arguments: &[Value]) -> Result<Value, RuntimeError> {
    car(arguments)
}

fn rest(arguments: &[Value]) -> Result<Value, RuntimeError> {
    cdr(arguments)
}

fn append(arguments: &[Value]) -> Result<Value, RuntimeError> {
    append_lists("append", arguments)
}

fn append_lists(function: &str, arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Ok(Value::Nil);
    }
    let mut values = Vec::new();
    for argument in &arguments[..arguments.len() - 1] {
        let Some(items) = argument.list_items() else {
            return Err(type_error(function, "list", argument));
        };
        values.extend(items);
    }
    match arguments.last().expect("arguments is non-empty") {
        Value::Nil | Value::List(_) => {
            let Some(items) = arguments.last().and_then(Value::list_items) else {
                unreachable!();
            };
            values.extend(items);
            Ok(Value::list(values))
        }
        Value::DottedList { items, tail } => {
            if values.is_empty() && items.is_empty() {
                return Ok(arguments.last().expect("arguments is non-empty").clone());
            }
            values.extend(items.iter().cloned());
            Ok(Value::dotted_list(values, tail.as_ref().clone()))
        }
        tail if values.is_empty() => Ok(tail.clone()),
        tail => Ok(Value::dotted_list(values, tail.clone())),
    }
}

fn nconc(arguments: &[Value]) -> Result<Value, RuntimeError> {
    append_lists("nconc", arguments)
}

fn revappend(arguments: &[Value]) -> Result<Value, RuntimeError> {
    revappend_like("revappend", arguments)
}

fn nreconc(arguments: &[Value]) -> Result<Value, RuntimeError> {
    revappend_like("nreconc", arguments)
}

fn revappend_like(function: &str, arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, function, 2)?;
    let Some(mut items) = arguments[0].list_items() else {
        return Err(type_error(function, "list", &arguments[0]));
    };
    items.reverse();
    let append_arguments = [Value::list(items), arguments[1].clone()];
    append_lists(function, &append_arguments)
}

fn length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "length", 1)?;
    let length = match &arguments[0] {
        Value::Nil => 0,
        Value::List(items) | Value::Vector(items) => items.len(),
        Value::String(value) => value.chars().count(),
        _ => {
            return Err(type_error("length", "sequence", &arguments[0]));
        }
    };
    Ok(Value::Integer(length as i64))
}

fn nth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "nth", 2)?;
    let Some(items) = arguments[1].list_items() else {
        return Err(type_error("nth", "list", &arguments[1]));
    };
    let index = index_argument("nth", &arguments[0])?;
    Ok(items.get(index).cloned().unwrap_or(Value::Nil))
}

fn elt(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "elt", 2)?;
    let index = index_argument("elt", &arguments[1])?;
    match &arguments[0] {
        Value::Nil => Err(out_of_bounds("elt", index)),
        Value::List(items) | Value::Vector(items) => items
            .get(index)
            .cloned()
            .ok_or_else(|| out_of_bounds("elt", index)),
        Value::String(value) => value
            .chars()
            .nth(index)
            .map(Value::Character)
            .ok_or_else(|| out_of_bounds("elt", index)),
        value => Err(type_error("elt", "sequence", value)),
    }
}

fn string_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "string", 1)?;
    Ok(Value::string(string_designator("string", &arguments[0])?))
}

fn make_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("make-string", "1 or 2", arguments.len()));
    }
    let length = index_argument("make-string", &arguments[0])?;
    let initial = arguments
        .get(1)
        .map(|value| character_argument("make-string", value))
        .transpose()?
        .unwrap_or(' ');
    Ok(Value::string(
        std::iter::repeat(initial).take(length).collect::<String>(),
    ))
}

fn character(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char", 2)?;
    let index = index_argument("char", &arguments[1])?;
    let Value::String(value) = &arguments[0] else {
        return Err(type_error("char", "string", &arguments[0]));
    };
    value
        .chars()
        .nth(index)
        .map(Value::Character)
        .ok_or_else(|| out_of_bounds("char", index))
}

fn char_code(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-code", 1)?;
    Ok(Value::Integer(
        character_argument("char-code", &arguments[0])? as i64,
    ))
}

fn code_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "code-char", 1)?;
    let code = integer_argument("code-char", &arguments[0])?;
    Ok(u32::try_from(code)
        .ok()
        .and_then(char::from_u32)
        .map(Value::Character)
        .unwrap_or(Value::Nil))
}

fn character_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char=", arguments, false, |left, right| left == right)
}

fn character_case_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-equal", arguments, true, |left, right| left == right)
}

fn character_less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char<", arguments, false, |left, right| left < right)
}

fn character_greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char>", arguments, false, |left, right| left > right)
}

fn character_less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char<=", arguments, false, |left, right| left <= right)
}

fn character_greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char>=", arguments, false, |left, right| left >= right)
}

fn compare_characters(
    function: &str,
    arguments: &[Value],
    ignore_case: bool,
    comparison: fn(char, char) -> bool,
) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity(function, "at least 2", arguments.len()));
    }
    let characters = arguments
        .iter()
        .map(|value| character_argument(function, value))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::boolean(characters.windows(2).all(|window| {
        let left = if ignore_case {
            window[0].to_ascii_lowercase()
        } else {
            window[0]
        };
        let right = if ignore_case {
            window[1].to_ascii_lowercase()
        } else {
            window[1]
        };
        comparison(left, right)
    })))
}

fn character_upcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-upcase", 1)?;
    Ok(Value::Character(
        character_argument("char-upcase", &arguments[0])?.to_ascii_uppercase(),
    ))
}

fn character_downcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-downcase", 1)?;
    Ok(Value::Character(
        character_argument("char-downcase", &arguments[0])?.to_ascii_lowercase(),
    ))
}

fn string_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_equality("string=", arguments, false)
}

fn string_case_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_equality("string-equal", arguments, true)
}

fn string_less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string<", arguments, false, |ordering| {
        ordering == Ordering::Less
    })
}

fn string_greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string>", arguments, false, |ordering| {
        ordering == Ordering::Greater
    })
}

fn string_less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string<=", arguments, false, |ordering| {
        ordering != Ordering::Greater
    })
}

fn string_greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string>=", arguments, false, |ordering| {
        ordering != Ordering::Less
    })
}

fn compare_strings(
    function: &str,
    arguments: &[Value],
    ignore_case: bool,
    comparison: fn(Ordering) -> bool,
) -> Result<Value, RuntimeError> {
    exact(arguments, function, 2)?;
    let left = string_designator(function, &arguments[0])?;
    let right = string_designator(function, &arguments[1])?;
    let (index, ordering) = string_order(&left, &right, ignore_case);
    if comparison(ordering) {
        Ok(Value::Integer(index as i64))
    } else {
        Ok(Value::Nil)
    }
}

fn string_equality(
    function: &str,
    arguments: &[Value],
    ignore_case: bool,
) -> Result<Value, RuntimeError> {
    exact(arguments, function, 2)?;
    let left = string_designator(function, &arguments[0])?;
    let right = string_designator(function, &arguments[1])?;
    let (_, ordering) = string_order(&left, &right, ignore_case);
    Ok(Value::boolean(ordering == Ordering::Equal))
}

fn string_order(left: &str, right: &str, ignore_case: bool) -> (usize, Ordering) {
    let left = left.chars().collect::<Vec<_>>();
    let right = right.chars().collect::<Vec<_>>();
    for (index, (left, right)) in left.iter().zip(&right).enumerate() {
        let left = if ignore_case {
            left.to_ascii_lowercase()
        } else {
            *left
        };
        let right = if ignore_case {
            right.to_ascii_lowercase()
        } else {
            *right
        };
        if left != right {
            return (index, left.cmp(&right));
        }
    }
    (left.len().min(right.len()), left.len().cmp(&right.len()))
}

fn string_upcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "string-upcase", StringCase::Upper)
}

fn string_downcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "string-downcase", StringCase::Lower)
}

fn string_capitalize(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "string-capitalize", StringCase::Capitalize)
}

fn nstring_upcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "nstring-upcase", StringCase::Upper)
}

fn nstring_downcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "nstring-downcase", StringCase::Lower)
}

fn nstring_capitalize(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "nstring-capitalize", StringCase::Capitalize)
}

#[derive(Clone, Copy)]
enum StringCase {
    Upper,
    Lower,
    Capitalize,
}

fn string_case_transform(
    arguments: &[Value],
    function: &str,
    case: StringCase,
) -> Result<Value, RuntimeError> {
    if !(1..=5).contains(&arguments.len()) || (arguments.len() - 1) % 2 != 0 {
        return Err(arity(function, "1, 3, or 5", arguments.len()));
    }
    let value = string_designator(function, &arguments[0])?;
    let characters = value.chars().collect::<Vec<_>>();
    let (start, end) = sequence_bounds(function, characters.len(), &arguments[1..])?;
    let mut output = String::new();
    let mut word_start = true;
    for (index, character) in characters.into_iter().enumerate() {
        let in_range = (start..end).contains(&index);
        match case {
            StringCase::Upper if in_range => output.extend(character.to_uppercase()),
            StringCase::Lower if in_range => output.extend(character.to_lowercase()),
            StringCase::Capitalize if character.is_alphanumeric() => {
                if in_range && word_start {
                    output.extend(character.to_uppercase());
                } else if in_range {
                    output.extend(character.to_lowercase());
                } else {
                    output.push(character);
                }
                word_start = false;
            }
            StringCase::Capitalize => {
                output.push(character);
                word_start = true;
            }
            _ => output.push(character),
        }
    }
    Ok(Value::string(output))
}

fn string_trim(arguments: &[Value]) -> Result<Value, RuntimeError> {
    trim_string(arguments, "string-trim", true, true)
}

fn string_left_trim(arguments: &[Value]) -> Result<Value, RuntimeError> {
    trim_string(arguments, "string-left-trim", true, false)
}

fn string_right_trim(arguments: &[Value]) -> Result<Value, RuntimeError> {
    trim_string(arguments, "string-right-trim", false, true)
}

fn trim_string(
    arguments: &[Value],
    function: &str,
    trim_left: bool,
    trim_right: bool,
) -> Result<Value, RuntimeError> {
    exact(arguments, function, 2)?;
    let trim_set = sequence_elements(function, &arguments[0])?
        .into_iter()
        .map(|value| character_argument(function, &value))
        .collect::<Result<Vec<_>, _>>()?;
    let value = string_designator(function, &arguments[1])?;
    let characters = value.chars().collect::<Vec<_>>();
    let is_trimmed = |character: &char| trim_set.contains(character);
    let start = if trim_left {
        characters.iter().position(|character| !is_trimmed(character))
    } else {
        Some(0)
    }
    .unwrap_or(characters.len());
    let end = if trim_right {
        characters
            .iter()
            .rposition(|character| !is_trimmed(character))
            .map_or(0, |index| index + 1)
    } else {
        characters.len()
    };
    Ok(Value::string(
        characters[start.min(end)..end].iter().collect::<String>(),
    ))
}

fn character_argument(function: &str, value: &Value) -> Result<char, RuntimeError> {
    match value {
        Value::Character(value) => Ok(*value),
        value => Err(type_error(function, "character", value)),
    }
}

fn string_designator(function: &str, value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Nil => Ok("NIL".to_string()),
        Value::Boolean(true) => Ok("T".to_string()),
        Value::Boolean(false) => Ok("NIL".to_string()),
        Value::String(value)
        | Value::Symbol(value)
        | Value::UninternedSymbol(value)
        | Value::Keyword(value)
        | Value::SymbolExact(value)
        | Value::KeywordExact(value) => {
            Ok(value.to_string())
        }
        Value::Character(value) => Ok(value.to_string()),
        value => Err(type_error(function, "string designator", value)),
    }
}

fn subseq(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&arguments.len()) {
        return Err(arity("subseq", "2 or 3", arguments.len()));
    }
    let length = sequence_length(&arguments[0])
        .ok_or_else(|| type_error("subseq", "sequence", &arguments[0]))?;
    let start = index_argument("subseq", &arguments[1])?;
    let end = arguments
        .get(2)
        .map(|value| index_argument("subseq", value))
        .transpose()?
        .unwrap_or(length);
    if start > end || end > length {
        return Err(RuntimeError::InvalidForm {
            message: "subseq bounds are invalid".to_string(),
            span: None,
        });
    }
    match &arguments[0] {
        Value::Nil => Ok(Value::Nil),
        Value::List(items) => Ok(Value::list(items[start..end].to_vec())),
        Value::Vector(items) => Ok(Value::vector(items[start..end].to_vec())),
        Value::String(value) => {
            let result = value
                .chars()
                .skip(start)
                .take(end - start)
                .collect::<String>();
            Ok(Value::string(result))
        }
        _ => Err(type_error("subseq", "sequence", &arguments[0])),
    }
}

fn fill(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity("fill", "at least two", arguments.len()));
    }
    if (arguments.len() - 2) % 2 != 0 {
        return Err(arity(
            "fill",
            "an item, a sequence, and keyword/value pairs",
            arguments.len(),
        ));
    }
    let length = sequence_length(&arguments[1])
        .ok_or_else(|| type_error("fill", "sequence", &arguments[1]))?;
    let (start, end) = sequence_bounds("fill", length, &arguments[2..])?;
    if matches!(arguments[1], Value::String(_))
        && !matches!(arguments[0], Value::Character(_))
    {
        return Err(type_error("fill", "a character for a string", &arguments[0]));
    }
    let mut items = sequence_elements("fill", &arguments[1])?;
    for item in &mut items[start..end] {
        *item = arguments[0].clone();
    }
    rebuild_sequence("fill", &arguments[1], items)
}

fn replace(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity("replace", "at least two", arguments.len()));
    }
    if (arguments.len() - 2) % 2 != 0 {
        return Err(arity(
            "replace",
            "two sequences and keyword/value pairs",
            arguments.len(),
        ));
    }
    let first_length = sequence_length(&arguments[0])
        .ok_or_else(|| type_error("replace", "sequence", &arguments[0]))?;
    let second_length = sequence_length(&arguments[1])
        .ok_or_else(|| type_error("replace", "sequence", &arguments[1]))?;
    let (start1, end1, start2, end2) =
        replace_bounds(first_length, second_length, &arguments[2..])?;
    let mut result = sequence_elements("replace", &arguments[0])?;
    let source = sequence_elements("replace", &arguments[1])?;
    let count = (end1 - start1).min(end2 - start2);
    if matches!(arguments[0], Value::String(_))
        && source[start2..start2 + count]
            .iter()
            .any(|value| !matches!(value, Value::Character(_)))
    {
        return Err(type_error(
            "replace",
            "characters in the source sequence for a string destination",
            &arguments[1],
        ));
    }
    for offset in 0..count {
        result[start1 + offset] = source[start2 + offset].clone();
    }
    rebuild_sequence("replace", &arguments[0], result)
}

fn copy_seq(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "copy-seq", 1)?;
    let items = sequence_elements("copy-seq", &arguments[0])?;
    rebuild_sequence("copy-seq", &arguments[0], items)
}

fn concatenate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("concatenate", "at least one", 0));
    }
    let result_type = type_designator_name("concatenate", &arguments[0])?;
    let mut items = Vec::new();
    for sequence in &arguments[1..] {
        items.extend(sequence_elements("concatenate", sequence)?);
    }
    match result_type.as_str() {
        "LIST" => Ok(Value::list(items)),
        "VECTOR" => Ok(Value::vector(items)),
        "STRING" | "SIMPLE-STRING" => {
            let mut result = String::new();
            for item in items {
                let Value::Character(character) = item else {
                    return Err(type_error(
                        "concatenate",
                        "characters for a string result",
                        &item,
                    ));
                };
                result.push(character);
            }
            Ok(Value::string(result))
        }
        _ => Err(RuntimeError::InvalidForm {
            message: format!(
                "concatenate result type must be LIST, VECTOR, or STRING, got {result_type}"
            ),
            span: None,
        }),
    }
}

fn make_sequence(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 || (arguments.len() - 2) % 2 != 0 {
        return Err(arity(
            "make-sequence",
            "a result type, a size, and keyword/value pairs",
            arguments.len(),
        ));
    }
    let result_type = type_designator_name("make-sequence", &arguments[0])?;
    let size = index_argument("make-sequence", &arguments[1])?;
    let mut initial_element = Value::Nil;
    for pair in arguments[2..].chunks_exact(2) {
        match array_option_name("make-sequence", &pair[0])?.as_str() {
            "INITIAL-ELEMENT" => initial_element = pair[1].clone(),
            option => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("make-sequence does not accept :{option}"),
                    span: None,
                });
            }
        }
    }
    match result_type.as_str() {
        "LIST" => Ok(Value::list(vec![initial_element; size])),
        "VECTOR" | "SIMPLE-VECTOR" => Ok(Value::vector(vec![initial_element; size])),
        "STRING" | "SIMPLE-STRING" => {
            let initial = character_argument("make-sequence", &initial_element)?;
            Ok(Value::string(
                std::iter::repeat(initial).take(size).collect::<String>(),
            ))
        }
        _ => Err(RuntimeError::InvalidForm {
            message: format!(
                "make-sequence result type must be LIST, VECTOR, or STRING, got {result_type}"
            ),
            span: None,
        }),
    }
}

fn coerce(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "coerce", 2)?;
    let result_type = type_designator_name("coerce", &arguments[1])?;
    match result_type.as_str() {
        "LIST" => Ok(Value::list(sequence_elements("coerce", &arguments[0])?)),
        "VECTOR" | "SIMPLE-VECTOR" => {
            Ok(Value::vector(sequence_elements("coerce", &arguments[0])?))
        }
        "STRING" | "SIMPLE-STRING" => {
            let result = match &arguments[0] {
                Value::Nil
                | Value::Boolean(_)
                | Value::String(_)
                | Value::Symbol(_)
                | Value::UninternedSymbol(_)
                | Value::Keyword(_)
                | Value::SymbolExact(_)
                | Value::KeywordExact(_)
                | Value::Character(_) => string_designator("coerce", &arguments[0])?,
                value => sequence_elements("coerce", value)?
                    .into_iter()
                    .map(|item| character_argument("coerce", &item))
                    .collect::<Result<String, RuntimeError>>()?,
            };
            Ok(Value::string(result))
        }
        "SEQUENCE" => match &arguments[0] {
            Value::Nil | Value::List(_) | Value::Vector(_) | Value::String(_) => {
                Ok(arguments[0].clone())
            }
            value => Err(type_error("coerce", "a sequence", value)),
        },
        "CHARACTER" => match &arguments[0] {
            Value::Character(_) => Ok(arguments[0].clone()),
            value => Err(type_error("coerce", "a character", value)),
        },
        _ => Err(RuntimeError::InvalidForm {
            message: format!("coerce does not support result type {result_type}"),
            span: None,
        }),
    }
}

fn sequence_bounds(
    function: &str,
    length: usize,
    options: &[Value],
) -> Result<(usize, usize), RuntimeError> {
    let mut start = 0;
    let mut end = length;
    for pair in options.chunks_exact(2) {
        match array_option_name(function, &pair[0])?.as_str() {
            "START" => start = index_argument(function, &pair[1])?,
            "END" => end = index_argument(function, &pair[1])?,
            option => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("{function} does not accept :{option}"),
                    span: None,
                });
            }
        }
    }
    if start > end || end > length {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} bounds are invalid"),
            span: None,
        });
    }
    Ok((start, end))
}

fn replace_bounds(
    first_length: usize,
    second_length: usize,
    options: &[Value],
) -> Result<(usize, usize, usize, usize), RuntimeError> {
    let mut start1 = 0;
    let mut end1 = first_length;
    let mut start2 = 0;
    let mut end2 = second_length;
    for pair in options.chunks_exact(2) {
        let option = array_option_name("replace", &pair[0])?;
        match option.as_str() {
            "START1" => start1 = index_argument("replace", &pair[1])?,
            "END1" => end1 = index_argument("replace", &pair[1])?,
            "START2" => start2 = index_argument("replace", &pair[1])?,
            "END2" => end2 = index_argument("replace", &pair[1])?,
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("replace does not accept :{option}"),
                    span: None,
                });
            }
        }
    }
    if start1 > end1 || end1 > first_length || start2 > end2 || end2 > second_length {
        return Err(RuntimeError::InvalidForm {
            message: "replace bounds are invalid".to_string(),
            span: None,
        });
    }
    Ok((start1, end1, start2, end2))
}

fn sequence_elements(function: &str, value: &Value) -> Result<Vec<Value>, RuntimeError> {
    match value {
        Value::Nil => Ok(Vec::new()),
        Value::List(items) | Value::Vector(items) => Ok(items.as_ref().clone()),
        Value::String(value) => Ok(value.chars().map(Value::Character).collect()),
        _ => Err(type_error(function, "sequence", value)),
    }
}

fn rebuild_sequence(
    function: &str,
    template: &Value,
    items: Vec<Value>,
) -> Result<Value, RuntimeError> {
    match template {
        Value::Nil | Value::List(_) => Ok(Value::list(items)),
        Value::Vector(_) => Ok(Value::vector(items)),
        Value::String(_) => {
            let mut result = String::new();
            for item in items {
                let Value::Character(character) = item else {
                    return Err(type_error(
                        function,
                        "characters for a string sequence",
                        &item,
                    ));
                };
                result.push(character);
            }
            Ok(Value::string(result))
        }
        value => Err(type_error(function, "sequence", value)),
    }
}

fn getf(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&arguments.len()) {
        return Err(arity("getf", "2 or 3", arguments.len()));
    }
    let Some(items) = arguments[0].list_items() else {
        return Err(type_error("getf", "property list", &arguments[0]));
    };
    if items.len() % 2 != 0 {
        return Err(RuntimeError::InvalidForm {
            message: "getf requires an even-length property list".to_string(),
            span: None,
        });
    }
    for pair in items.chunks_exact(2) {
        if arguments[1].eq_value(&pair[0]) {
            return Ok(pair[1].clone());
        }
    }
    Ok(arguments.get(2).cloned().unwrap_or(Value::Nil))
}

fn get_properties(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "get-properties", 2)?;
    let Some(plist) = arguments[0].list_items() else {
        return Err(type_error("get-properties", "property list", &arguments[0]));
    };
    let Some(indicators) = arguments[1].list_items() else {
        return Err(type_error("get-properties", "list", &arguments[1]));
    };
    if plist.len() % 2 != 0 {
        return Err(RuntimeError::InvalidForm {
            message: "get-properties requires an even-length property list".to_string(),
            span: None,
        });
    }
    for (index, pair) in plist.chunks_exact(2).enumerate() {
        if indicators
            .iter()
            .any(|indicator| indicator.eq_value(&pair[0]))
        {
            return Ok(Value::values(vec![
                pair[0].clone(),
                pair[1].clone(),
                Value::list(plist[index * 2..].to_vec()),
            ]));
        }
    }
    Ok(Value::values(vec![Value::Nil, Value::Nil, Value::Nil]))
}

fn sequence_length(value: &Value) -> Option<usize> {
    match value {
        Value::Nil => Some(0),
        Value::List(items) | Value::Vector(items) => Some(items.len()),
        Value::String(value) => Some(value.chars().count()),
        _ => None,
    }
}

fn index_argument(function: &str, value: &Value) -> Result<usize, RuntimeError> {
    let index = integer_argument(function, value)?;
    usize::try_from(index).map_err(|_| RuntimeError::InvalidForm {
        message: format!("{function} index must be non-negative"),
        span: None,
    })
}

fn out_of_bounds(function: &str, index: usize) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: format!("{function} index {index} is out of bounds"),
        span: None,
    }
}

fn endp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "endp", 1)?;
    match &arguments[0] {
        Value::Nil => Ok(Value::boolean(true)),
        Value::List(_) => Ok(Value::boolean(false)),
        value => Err(type_error("endp", "list", value)),
    }
}

fn characterp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "characterp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Character(_))))
}

fn keywordp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "keywordp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Keyword(_) | Value::KeywordExact(_)
    )))
}

fn symbol_name_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "symbol-name", 1)?;
    let name = match &arguments[0] {
        Value::UninternedSymbol(name) => name.to_string(),
        value => {
            let name = value
                .symbol_name()
                .ok_or_else(|| type_error("symbol-name", "a symbol", &arguments[0]))?;
            let name = match package::split_symbol(name) {
                Some((_, symbol_name, _)) => symbol_name,
                None => name,
            };
            name.to_string()
        }
    };
    Ok(Value::string(name))
}

fn symbol_package_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "symbol-package", 1)?;
    let package_name = match &arguments[0] {
        Value::UninternedSymbol(_) => return Ok(Value::Nil),
        Value::Keyword(_) | Value::KeywordExact(_) => KEYWORD_PACKAGE.to_string(),
        Value::Nil | Value::Boolean(_) => COMMON_LISP_PACKAGE.to_string(),
        Value::Symbol(name) | Value::SymbolExact(name) => match package::split_symbol(name.as_ref()) {
            Some((package_name, _, _)) => package::normalize_package_name(package_name),
            None => package::DEFAULT_PACKAGE.to_string(),
        },
        value => return Err(type_error("symbol-package", "a symbol", value)),
    };
    Ok(Value::symbol(package_name))
}

fn vectorp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "vectorp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Vector(_))))
}

fn simple_vector_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-vector-p", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Vector(_))))
}

fn typep(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "typep", 2)?;
    Ok(Value::boolean(typep_value(&arguments[0], &arguments[1])?))
}

pub(crate) fn typep_value(value: &Value, type_designator: &Value) -> Result<bool, RuntimeError> {
    let type_name = type_designator_name("typep", type_designator)?;
    type_matches(value, &type_name)
}

pub(crate) fn the_check(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "the", 2)?;
    let type_name = type_designator_name("the", &arguments[1])?;
    if type_matches(&arguments[0], &type_name)? {
        Ok(arguments[0].clone())
    } else {
        Err(RuntimeError::Type {
            expected: format!("the requires value of type {type_name}"),
            actual: arguments[0].type_name().to_string(),
            span: None,
        })
    }
}

fn ecase_error(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "__NCL_ECASE_ERROR", 0)?;
    Err(RuntimeError::InvalidForm {
        message: "ecase fell through".to_string(),
        span: None,
    })
}

fn etypecase_error(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "__NCL_ETYPECASE_ERROR", 0)?;
    Err(RuntimeError::InvalidForm {
        message: "etypecase fell through".to_string(),
        span: None,
    })
}

fn type_designator_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    let type_name = match value {
        Value::Symbol(name)
        | Value::UninternedSymbol(name)
        | Value::Keyword(name)
        | Value::SymbolExact(name)
        | Value::KeywordExact(name) => name.as_ref(),
        value => return Err(type_error(function, "type designator", value)),
    };
    let type_name = type_name.rsplit("::").next().unwrap_or(type_name);
    Ok(package::normalize_symbol_name(type_name))
}

fn type_matches(value: &Value, type_name: &str) -> Result<bool, RuntimeError> {
    let result = match type_name {
        "T" | "OBJECT" => true,
        "NIL" | "NULL" => matches!(value, Value::Nil),
        "BOOLEAN" => matches!(value, Value::Boolean(_)),
        "NUMBER" | "REAL" => matches!(
            value,
            Value::Integer(_) | Value::Rational(_) | Value::Float(_)
        ),
        "RATIONAL" => matches!(value, Value::Integer(_) | Value::Rational(_)),
        "RATIO" => matches!(value, Value::Rational(_)),
        "INTEGER" => matches!(value, Value::Integer(_)),
        "FLOAT" => matches!(value, Value::Float(_)),
        "CHARACTER" => matches!(value, Value::Character(_)),
        "STRING" | "SIMPLE-STRING" => matches!(value, Value::String(_)),
        "STREAM" => matches!(value, Value::Stream(_)),
        "SYMBOL" => matches!(
            value,
            Value::Nil
                | Value::Boolean(_)
                | Value::Symbol(_)
                | Value::UninternedSymbol(_)
                | Value::Keyword(_)
                | Value::SymbolExact(_)
                | Value::KeywordExact(_)
        ),
        "PACKAGE" => matches!(value, Value::Package(_)),
        "KEYWORD" => matches!(value, Value::Keyword(_) | Value::KeywordExact(_)),
        "CONS" => matches!(value, Value::List(_) | Value::DottedList { .. }),
        "LIST" => matches!(value, Value::Nil | Value::List(_)),
        "ATOM" => !matches!(value, Value::List(_) | Value::DottedList { .. }),
        "VECTOR" => matches!(value, Value::Vector(_)),
        "ARRAY" | "SIMPLE-ARRAY" => dimensions_for_array(value).is_some(),
        "HASH-TABLE" => matches!(value, Value::HashTable { .. }),
        "CONDITION" => matches!(value, Value::Condition(_)),
        "STRUCTURE" => value.structure_name().is_some(),
        "SEQUENCE" => sequence_length(value).is_some(),
        "FUNCTION" => matches!(value, Value::Function(_)),
        _ if value.instance_is_type(type_name) => true,
        _ if value.structure_is_type(type_name) => true,
        _ => {
            return Err(RuntimeError::InvalidForm {
                message: format!("unknown type designator {type_name}"),
                span: None,
            });
        }
    };
    Ok(result)
}

fn reverse(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "reverse", 1)?;
    reverse_list("reverse", &arguments[0])
}

fn nreverse(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "nreverse", 1)?;
    reverse_list("nreverse", &arguments[0])
}

fn reverse_list(function: &str, value: &Value) -> Result<Value, RuntimeError> {
    let Some(mut items) = value.list_items() else {
        return Err(type_error(function, "list", value));
    };
    items.reverse();
    Ok(Value::list(items))
}

fn last(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("last", "one or two", arguments.len()));
    }
    let Some(items) = arguments[0].list_items() else {
        return Err(type_error("last", "list", &arguments[0]));
    };
    let count = arguments
        .get(1)
        .map(|value| index_argument("last", value))
        .transpose()?
        .unwrap_or(1);
    if count == 0 {
        return Ok(Value::Nil);
    }
    let start = items.len().saturating_sub(count);
    Ok(Value::list(items[start..].to_vec()))
}

fn butlast(arguments: &[Value]) -> Result<Value, RuntimeError> {
    butlast_like("butlast", arguments)
}

fn nbutlast(arguments: &[Value]) -> Result<Value, RuntimeError> {
    butlast_like("nbutlast", arguments)
}

fn butlast_like(function: &str, arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity(function, "one or two", arguments.len()));
    }
    let Some(items) = arguments[0].list_items() else {
        return Err(type_error(function, "list", &arguments[0]));
    };
    let count = arguments
        .get(1)
        .map(|value| index_argument(function, value))
        .transpose()?
        .unwrap_or(1);
    let end = items.len().saturating_sub(count);
    Ok(Value::list(items[..end].to_vec()))
}

fn copy_list(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "copy-list", 1)?;
    let Some(items) = arguments[0].list_items() else {
        return Err(type_error("copy-list", "list", &arguments[0]));
    };
    Ok(Value::list(items))
}

fn copy_alist(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "copy-alist", 1)?;
    let Some(entries) = arguments[0].list_items() else {
        return Err(type_error("copy-alist", "association list", &arguments[0]));
    };
    let copied = entries
        .into_iter()
        .map(|entry| match entry {
            Value::List(items) => Ok(Value::list(items.as_ref().clone())),
            Value::DottedList { items, tail } => {
                Ok(Value::dotted_list(items.as_ref().clone(), tail.as_ref().clone()))
            }
            value => Err(type_error("copy-alist", "association", &value)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::list(copied))
}

fn copy_tree(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "copy-tree", 1)?;
    Ok(copy_tree_value(&arguments[0]))
}

fn copy_tree_value(value: &Value) -> Value {
    match value {
        Value::List(items) => Value::list(items.iter().map(copy_tree_value).collect()),
        Value::DottedList { items, tail } => Value::dotted_list(
            items.iter().map(copy_tree_value).collect(),
            copy_tree_value(tail),
        ),
        _ => value.clone(),
    }
}

fn vector(arguments: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::vector(arguments.to_vec()))
}

fn make_array(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("make-array", "at least one", 0));
    }
    let dimensions = parse_array_dimensions("make-array", &arguments[0])?;
    let mut initial_element = None;
    let mut initial_contents = None;
    if (arguments.len() - 1) % 2 != 0 {
        return Err(arity(
            "make-array",
            "one dimension and keyword/value pairs",
            arguments.len(),
        ));
    }
    for pair in arguments[1..].chunks_exact(2) {
        let name = array_option_name("make-array", &pair[0])?;
        match name.as_str() {
            "INITIAL-ELEMENT" => {
                if initial_contents.is_some() {
                    return Err(RuntimeError::InvalidForm {
                        message: "make-array cannot combine :initial-element and :initial-contents"
                            .to_string(),
                        span: None,
                    });
                }
                initial_element = Some(pair[1].clone());
            }
            "INITIAL-CONTENTS" => {
                if initial_element.is_some() {
                    return Err(RuntimeError::InvalidForm {
                        message: "make-array cannot combine :initial-element and :initial-contents"
                            .to_string(),
                        span: None,
                    });
                }
                initial_contents = Some(pair[1].clone());
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("make-array does not support keyword :{name}"),
                    span: None,
                });
            }
        }
    }
    let total_size = array_total_size_for("make-array", &dimensions)?;
    let elements = if let Some(contents) = initial_contents {
        let mut elements = Vec::with_capacity(total_size);
        flatten_array_contents("make-array", &contents, &dimensions, &mut elements)?;
        elements
    } else {
        vec![initial_element.unwrap_or(Value::Nil); total_size]
    };
    if dimensions.len() == 1 {
        Ok(Value::vector(elements))
    } else {
        Ok(Value::array(dimensions, elements))
    }
}

fn aref(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("aref", "at least one", 0));
    }
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("aref", "array", &arguments[0]))?;
    if arguments.len() != dimensions.len() + 1 {
        return Err(arity(
            "aref",
            (dimensions.len() + 1).to_string(),
            arguments.len(),
        ));
    }
    let index = array_coordinate_index("aref", &dimensions, &arguments[1..])?;
    array_elements(&arguments[0])
        .and_then(|items| items.get(index).cloned())
        .ok_or_else(|| out_of_bounds("aref", index))
}

fn row_major_aref(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "row-major-aref", 2)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("row-major-aref", "array", &arguments[0]))?;
    let index = index_argument("row-major-aref", &arguments[1])?;
    let total_size = array_total_size_for("row-major-aref", &dimensions)?;
    if index >= total_size {
        return Err(out_of_bounds("row-major-aref", index));
    }
    array_elements(&arguments[0])
        .and_then(|items| items.get(index).cloned())
        .ok_or_else(|| out_of_bounds("row-major-aref", index))
}

fn array_row_major_index(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("array-row-major-index", "array and subscripts", 0));
    }
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-row-major-index", "array", &arguments[0]))?;
    if arguments.len() != dimensions.len() + 1 {
        return Err(arity(
            "array-row-major-index",
            (dimensions.len() + 1).to_string(),
            arguments.len(),
        ));
    }
    Ok(Value::Integer(
        array_coordinate_index("array-row-major-index", &dimensions, &arguments[1..])? as i64,
    ))
}

fn array_in_bounds_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("array-in-bounds-p", "array and subscripts", 0));
    }
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-in-bounds-p", "array", &arguments[0]))?;
    if arguments.len() != dimensions.len() + 1 {
        return Err(arity(
            "array-in-bounds-p",
            (dimensions.len() + 1).to_string(),
            arguments.len(),
        ));
    }
    for (dimension, value) in dimensions.iter().zip(&arguments[1..]) {
        if index_argument("array-in-bounds-p", value)? >= *dimension {
            return Ok(Value::Nil);
        }
    }
    Ok(Value::boolean(true))
}

fn array_element_type(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-element-type", 1)?;
    dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-element-type", "array", &arguments[0]))?;
    Ok(Value::symbol("T"))
}

fn simple_array_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-array-p", 1)?;
    Ok(Value::boolean(
        dimensions_for_array(&arguments[0]).is_some(),
    ))
}

fn arrayp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "arrayp", 1)?;
    Ok(Value::boolean(
        dimensions_for_array(&arguments[0]).is_some(),
    ))
}

fn array_rank(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-rank", 1)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-rank", "array", &arguments[0]))?;
    Ok(Value::Integer(dimensions.len() as i64))
}

fn array_dimensions(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-dimensions", 1)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-dimensions", "array", &arguments[0]))?;
    Ok(Value::list(
        dimensions
            .into_iter()
            .map(|dimension| Value::Integer(dimension as i64))
            .collect(),
    ))
}

fn array_dimension(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-dimension", 2)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-dimension", "array", &arguments[0]))?;
    let index = index_argument("array-dimension", &arguments[1])?;
    dimensions
        .get(index)
        .copied()
        .map(|dimension| Value::Integer(dimension as i64))
        .ok_or_else(|| out_of_bounds("array-dimension", index))
}

fn array_total_size(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-total-size", 1)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-total-size", "array", &arguments[0]))?;
    Ok(Value::Integer(
        array_total_size_for("array-total-size", &dimensions)? as i64,
    ))
}

fn make_hash_table(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() % 2 != 0 {
        return Err(arity(
            "make-hash-table",
            "keyword/value pairs",
            arguments.len(),
        ));
    }
    let mut test = "EQL".to_string();
    for pair in arguments.chunks_exact(2) {
        let name = hash_table_option_name("make-hash-table", &pair[0])?;
        match name.as_str() {
            "TEST" => test = hash_table_test_name("make-hash-table", &pair[1])?,
            "SIZE" => {
                index_argument("make-hash-table", &pair[1])?;
            }
            "REHASH-SIZE" => {
                let value = number_argument("make-hash-table", &pair[1])?;
                if value.as_float() <= 0.0 {
                    return Err(RuntimeError::InvalidForm {
                        message: "make-hash-table :rehash-size must be positive".to_string(),
                        span: None,
                    });
                }
            }
            "REHASH-THRESHOLD" => {
                let value = number_argument("make-hash-table", &pair[1])?.as_float();
                if !(0.0..=1.0).contains(&value) {
                    return Err(RuntimeError::InvalidForm {
                        message: "make-hash-table :rehash-threshold must be between 0 and 1"
                            .to_string(),
                        span: None,
                    });
                }
            }
            "SYNCHRONIZED" => {
                if !matches!(pair[1], Value::Nil | Value::Boolean(_)) {
                    return Err(type_error(
                        "make-hash-table",
                        "boolean for :synchronized",
                        &pair[1],
                    ));
                }
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("make-hash-table does not support keyword :{name}"),
                    span: None,
                });
            }
        }
    }
    Ok(Value::hash_table(test))
}

fn gethash(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() != 2 && arguments.len() != 3 {
        return Err(arity("gethash", "two or three", arguments.len()));
    }
    let table = &arguments[1];
    let Some(test) = table.hash_table_test() else {
        return Err(type_error("gethash", "hash-table", table));
    };
    let test = test.to_string();
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("gethash", "hash-table", table));
    };
    let key = &arguments[0];
    let found = entries
        .borrow()
        .iter()
        .find(|(stored_key, _)| hash_table_key_equal(&test, stored_key, key))
        .map(|(_, value)| value.clone());
    match found {
        Some(value) => Ok(Value::values(vec![value, Value::boolean(true)])),
        None => Ok(Value::values(vec![
            arguments.get(2).cloned().unwrap_or(Value::Nil),
            Value::Nil,
        ])),
    }
}

fn remhash(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "remhash", 2)?;
    let table = &arguments[1];
    let Some(test) = table.hash_table_test() else {
        return Err(type_error("remhash", "hash-table", table));
    };
    let test = test.to_string();
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("remhash", "hash-table", table));
    };
    let key = &arguments[0];
    let mut entries = entries.borrow_mut();
    let previous_length = entries.len();
    entries.retain(|(stored_key, _)| !hash_table_key_equal(&test, stored_key, key));
    Ok(Value::boolean(entries.len() != previous_length))
}

fn clrhash(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "clrhash", 1)?;
    let table = &arguments[0];
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("clrhash", "hash-table", table));
    };
    entries.borrow_mut().clear();
    Ok(table.clone())
}

fn hash_table_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-p", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::HashTable { .. }
    )))
}

fn hash_table_count(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-count", 1)?;
    let table = &arguments[0];
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("hash-table-count", "hash-table", table));
    };
    Ok(Value::Integer(entries.borrow().len() as i64))
}

fn hash_table_test_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-test", 1)?;
    let table = &arguments[0];
    let Some(test) = table.hash_table_test() else {
        return Err(type_error("hash-table-test", "hash-table", table));
    };
    Ok(Value::symbol(test))
}

fn hash_table_option_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Keyword(name)
        | Value::Symbol(name)
        | Value::UninternedSymbol(name)
        | Value::SymbolExact(name)
        | Value::KeywordExact(name) => {
            Ok(normalize_name(name))
        }
        other => Err(type_error(function, "keyword", other)),
    }
}

fn hash_table_test_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    let name = match value {
        Value::Keyword(name)
        | Value::Symbol(name)
        | Value::UninternedSymbol(name)
        | Value::SymbolExact(name)
        | Value::KeywordExact(name) => {
            normalize_name(name)
        }
        Value::Function(function_value) => match function_value.as_ref() {
            Function::Builtin { name, .. } | Function::Primitive { name } => normalize_name(name),
            _ => {
                return Err(type_error(
                    function,
                    "named hash-table test function",
                    value,
                ));
            }
        },
        other => return Err(type_error(function, "hash-table test designator", other)),
    };
    if matches!(name.as_str(), "EQ" | "EQL" | "EQUAL" | "EQUALP") {
        Ok(name)
    } else {
        Err(RuntimeError::InvalidForm {
            message: format!("{function} :test must be EQ, EQL, EQUAL, or EQUALP, got {name}"),
            span: None,
        })
    }
}

pub(crate) fn hash_table_key_equal(test: &str, left: &Value, right: &Value) -> bool {
    match test {
        "EQ" => left.eq_value(right),
        "EQUAL" => left.equal_value(right),
        "EQUALP" => equalp_value(left, right),
        _ => eql_value(left, right),
    }
}

fn parse_array_dimensions(function: &str, value: &Value) -> Result<Vec<usize>, RuntimeError> {
    match value {
        Value::Integer(_) => Ok(vec![index_argument(function, value)?]),
        Value::Nil => Ok(Vec::new()),
        Value::List(_) | Value::Vector(_) => {
            let items = sequence_items(value).expect("list or vector has sequence items");
            items
                .iter()
                .map(|item| index_argument(function, item))
                .collect()
        }
        other => Err(type_error(
            function,
            "integer or sequence of integers",
            other,
        )),
    }
}

fn array_option_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Keyword(name)
        | Value::Symbol(name)
        | Value::UninternedSymbol(name)
        | Value::SymbolExact(name)
        | Value::KeywordExact(name) => {
            Ok(normalize_name(name))
        }
        other => Err(type_error(function, "keyword", other)),
    }
}

fn flatten_array_contents(
    function: &str,
    contents: &Value,
    dimensions: &[usize],
    output: &mut Vec<Value>,
) -> Result<(), RuntimeError> {
    if dimensions.is_empty() {
        output.push(contents.clone());
        return Ok(());
    }
    let Some(items) = sequence_items(contents) else {
        return Err(type_error(
            function,
            "nested sequence for :initial-contents",
            contents,
        ));
    };
    if items.len() != dimensions[0] {
        return Err(RuntimeError::InvalidForm {
            message: format!(
                "{function} :initial-contents expected {} elements, got {}",
                dimensions[0],
                items.len()
            ),
            span: None,
        });
    }
    if dimensions.len() == 1 {
        output.extend(items);
    } else {
        for item in items {
            flatten_array_contents(function, &item, &dimensions[1..], output)?;
        }
    }
    Ok(())
}

fn array_coordinate_index(
    function: &str,
    dimensions: &[usize],
    indices: &[Value],
) -> Result<usize, RuntimeError> {
    let mut offset: usize = 0;
    for (dimension, value) in dimensions.iter().zip(indices) {
        let index = index_argument(function, value)?;
        if index >= *dimension {
            return Err(out_of_bounds(function, index));
        }
        offset = offset
            .checked_mul(*dimension)
            .and_then(|offset| offset.checked_add(index))
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: format!("{function} index is too large"),
                span: None,
            })?;
    }
    Ok(offset)
}

fn array_total_size_for(function: &str, dimensions: &[usize]) -> Result<usize, RuntimeError> {
    dimensions.iter().try_fold(1_usize, |total, dimension| {
        total
            .checked_mul(*dimension)
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: format!("{function} array is too large"),
                span: None,
            })
    })
}

fn dimensions_for_array(value: &Value) -> Option<Vec<usize>> {
    match value {
        Value::Vector(items) => Some(vec![items.len()]),
        Value::Array { dimensions, .. } => Some(dimensions.as_ref().clone()),
        _ => None,
    }
}

fn array_elements(value: &Value) -> Option<Vec<Value>> {
    value.vector_items().or_else(|| value.array_items())
}

fn sequence_items(value: &Value) -> Option<Vec<Value>> {
    value.list_items().or_else(|| value.vector_items())
}

fn null(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "null", 1)?;
    Ok(Value::boolean(!arguments[0].is_truthy()))
}

fn atom(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "atom", 1)?;
    Ok(Value::boolean(!matches!(
        &arguments[0],
        Value::List(_) | Value::DottedList { .. }
    )))
}

fn consp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "consp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::List(_) | Value::DottedList { .. }
    )))
}

fn listp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "listp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Nil | Value::List(_)
    )))
}

fn numberp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "numberp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Integer(_) | Value::Rational(_) | Value::Float(_)
    )))
}

fn integerp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "integerp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Integer(_))))
}

fn floatp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "floatp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Float(_))))
}

fn rationalp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rationalp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Integer(_) | Value::Rational(_)
    )))
}

fn stringp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "stringp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::String(_))))
}

fn simple_string_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-string-p", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::String(_))))
}

fn symbolp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "symbolp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Nil
            | Value::Boolean(_)
            | Value::Symbol(_)
            | Value::UninternedSymbol(_)
            | Value::Keyword(_)
            | Value::SymbolExact(_)
            | Value::KeywordExact(_)
    )))
}

fn packagep(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "packagep", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Package(_))))
}

fn functionp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "functionp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Function(_))))
}

fn eq(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "eq", 2)?;
    Ok(Value::boolean(arguments[0].eq_value(&arguments[1])))
}

fn eql(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "eql", 2)?;
    Ok(Value::boolean(eql_value(&arguments[0], &arguments[1])))
}

pub(crate) fn eql_value(left: &Value, right: &Value) -> bool {
    let numeric_equal = match (left, right) {
        (Value::Integer(left), Value::Integer(right)) => left == right,
        (Value::Rational(left), Value::Rational(right)) => left == right,
        (Value::Float(left), Value::Float(right)) => left == right,
        _ => false,
    };
    left.eq_value(right) || numeric_equal
}

fn equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "equal", 2)?;
    Ok(Value::boolean(arguments[0].equal_value(&arguments[1])))
}

fn equalp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "equalp", 2)?;
    Ok(Value::boolean(equalp_value(&arguments[0], &arguments[1])))
}

fn equalp_value(left: &Value, right: &Value) -> bool {
    if let (Ok(left), Ok(right)) = (number(left), number(right)) {
        return numeric_equalp(left, right);
    }
    match (left, right) {
        (Value::String(left), Value::String(right)) => left.eq_ignore_ascii_case(right),
        (Value::Character(left), Value::Character(right)) => {
            left.to_ascii_uppercase() == right.to_ascii_uppercase()
        }
        (Value::List(left), Value::List(right)) | (Value::Vector(left), Value::Vector(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| equalp_value(left, right))
        }
        (
            Value::Array {
                dimensions: left_dimensions,
                elements: left_elements,
            },
            Value::Array {
                dimensions: right_dimensions,
                elements: right_elements,
            },
        ) => {
            left_dimensions == right_dimensions
                && left_elements.len() == right_elements.len()
                && left_elements
                    .iter()
                    .zip(right_elements.iter())
                    .all(|(left, right)| equalp_value(left, right))
        }
        (
            Value::DottedList {
                items: left,
                tail: left_tail,
            },
            Value::DottedList {
                items: right,
                tail: right_tail,
            },
        ) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| equalp_value(left, right))
                && equalp_value(left_tail, right_tail)
        }
        _ => eql_value(left, right),
    }
}

fn identity(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "identity", 1)?;
    Ok(arguments[0].clone())
}

fn type_of(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "type-of", 1)?;
    Ok(Value::symbol(
        arguments[0]
            .structure_name()
            .unwrap_or(arguments[0].type_name()),
    ))
}

fn print_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("print", "1 to 2", arguments.len()));
    }
    write_destination("print", arguments.get(1), "\n")?;
    write_destination("print", arguments.get(1), &arguments[0].to_string())?;
    write_destination("print", arguments.get(1), "\n")?;
    Ok(arguments[0].clone())
}

fn princ(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("princ", "1 to 2", arguments.len()));
    }
    let text = match &arguments[0] {
        Value::String(value) => value.to_string(),
        value => value.to_string(),
    };
    write_destination("princ", arguments.get(1), &text)?;
    Ok(arguments[0].clone())
}

fn prin1(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("prin1", "1 to 2", arguments.len()));
    }
    write_destination("prin1", arguments.get(1), &arguments[0].to_string())?;
    Ok(arguments[0].clone())
}

fn write_to_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "write-to-string", 1)?;
    Ok(Value::string(arguments[0].to_string()))
}

fn read_from_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=3).contains(&arguments.len()) {
        return Err(arity("read-from-string", "1 to 3", arguments.len()));
    }
    let source = match &arguments[0] {
        Value::String(value) => value.as_ref(),
        value => return Err(type_error("read-from-string", "a string", value)),
    };
    let eof_error_p = arguments.get(1).map_or(true, Value::is_truthy);
    let eof_value = arguments.get(2).cloned().unwrap_or(Value::Nil);
    let mut reader = Reader::new(source);
    let (value, byte_position) = match reader.read_form()? {
        Some(form) => (quoted_form_value(&form)?, form.span.end),
        None => {
            let position = reader.position();
            if eof_error_p {
                return Err(RuntimeError::Read(ReadError::new(
                    ReadErrorKind::UnexpectedEnd { context: "a form" },
                    Span::new(position, position),
                )));
            }
            (eof_value, position)
        }
    };
    let position = source[..byte_position].chars().count();
    let position = i64::try_from(position).map_err(|_| RuntimeError::NumericOverflow)?;
    Ok(Value::values(vec![value, Value::Integer(position)]))
}

fn make_string_input_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=3).contains(&arguments.len()) {
        return Err(arity("make-string-input-stream", "1 to 3", arguments.len()));
    }
    let source = match &arguments[0] {
        Value::String(value) => value.as_ref(),
        value => return Err(type_error("make-string-input-stream", "a string", value)),
    };
    let length = source.chars().count();
    let start = match arguments.get(1) {
        Some(value) => stream_bound("make-string-input-stream", value, length)?,
        None => 0,
    };
    let end = match arguments.get(2) {
        Some(value) => stream_bound("make-string-input-stream", value, length)?,
        None => length,
    };
    if start > end {
        return Err(RuntimeError::InvalidForm {
            message: "make-string-input-stream start must not exceed end".to_string(),
            span: None,
        });
    }
    Ok(Value::string_input_stream(source, start, end))
}

fn stream_bound(function: &str, value: &Value, length: usize) -> Result<usize, RuntimeError> {
    let bound = integer_argument(function, value)?;
    let bound = usize::try_from(bound).map_err(|_| RuntimeError::InvalidForm {
        message: format!("{function} stream position must be non-negative"),
        span: None,
    })?;
    if bound > length {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} stream position is outside the string"),
            span: None,
        });
    }
    Ok(bound)
}

fn make_string_output_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "make-string-output-stream", 0)?;
    Ok(Value::string_output_stream())
}

fn stream_reference<'a>(
    function: &str,
    value: &'a Value,
) -> Result<&'a Rc<RefCell<Stream>>, RuntimeError> {
    match value {
        Value::Stream(stream) => Ok(stream),
        value => Err(type_error(function, "a stream", value)),
    }
}

fn stream_state_error(function: &str, expected: &str) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: format!("{function} requires {expected}"),
        span: None,
    }
}

fn get_output_stream_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "get-output-stream-string", 1)?;
    let stream = stream_reference("get-output-stream-string", &arguments[0])?;
    let output = stream
        .borrow_mut()
        .take_output()
        .ok_or_else(|| stream_state_error("get-output-stream-string", "an output stream"))?;
    Ok(Value::string(output))
}

fn read_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "read-char", 1)?;
    let stream = stream_reference("read-char", &arguments[0])?;
    let mut stream = stream.borrow_mut();
    if !stream.is_input() {
        return Err(stream_state_error("read-char", "an input stream"));
    }
    Ok(stream.read_char().map_or(Value::Nil, Value::Character))
}

fn peek_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "peek-char", 1)?;
    let stream = stream_reference("peek-char", &arguments[0])?;
    let stream = stream.borrow();
    if !stream.is_input() {
        return Err(stream_state_error("peek-char", "an input stream"));
    }
    Ok(stream.peek_char().map_or(Value::Nil, Value::Character))
}

fn unread_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "unread-char", 2)?;
    let character = match arguments[0] {
        Value::Character(character) => character,
        ref value => return Err(type_error("unread-char", "a character", value)),
    };
    let stream = stream_reference("unread-char", &arguments[1])?;
    let mut stream = stream.borrow_mut();
    if !stream.is_input() {
        return Err(stream_state_error("unread-char", "an input stream"));
    }
    if !stream.unread_char(character) {
        return Err(stream_state_error(
            "unread-char",
            "the last character read from an open input stream",
        ));
    }
    Ok(Value::Nil)
}

fn read_line(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "read-line", 1)?;
    let stream = stream_reference("read-line", &arguments[0])?;
    let mut stream = stream.borrow_mut();
    if !stream.is_input() {
        return Err(stream_state_error("read-line", "an input stream"));
    }
    match stream.read_line() {
        Some((line, eof)) => Ok(Value::values(vec![Value::string(line), Value::boolean(eof)])),
        None => Ok(Value::values(vec![Value::Nil, Value::boolean(true)])),
    }
}

fn write_destination(
    function: &str,
    destination: Option<&Value>,
    text: &str,
) -> Result<(), RuntimeError> {
    match destination {
        None | Some(Value::Nil) | Some(Value::Boolean(true)) => {
            print!("{text}");
            Ok(())
        }
        Some(Value::Stream(stream)) => {
            if stream.borrow_mut().write(text) {
                Ok(())
            } else {
                Err(stream_state_error(function, "an open output stream"))
            }
        }
        Some(value) => Err(type_error(
            function,
            "NIL, T, or an output stream",
            value,
        )),
    }
}

fn write_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("write-char", "1 to 2", arguments.len()));
    }
    let character = match arguments[0] {
        Value::Character(character) => character,
        ref value => return Err(type_error("write-char", "a character", value)),
    };
    write_destination("write-char", arguments.get(1), &character.to_string())?;
    Ok(Value::Character(character))
}

fn write_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("write-string", "1 to 2", arguments.len()));
    }
    let string = match &arguments[0] {
        Value::String(value) => value,
        value => return Err(type_error("write-string", "a string", value)),
    };
    write_destination("write-string", arguments.get(1), string)?;
    Ok(arguments[0].clone())
}

fn terpri(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 {
        return Err(arity("terpri", "0 to 1", arguments.len()));
    }
    write_destination("terpri", arguments.first(), "\n")?;
    Ok(Value::Nil)
}

fn fresh_line(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 {
        return Err(arity("fresh-line", "0 to 1", arguments.len()));
    }
    match arguments.first() {
        None | Some(Value::Nil) | Some(Value::Boolean(true)) => {
            println!();
            Ok(Value::boolean(true))
        }
        Some(Value::Stream(stream)) => stream
            .borrow_mut()
            .fresh_line()
            .map(Value::boolean)
            .ok_or_else(|| stream_state_error("fresh-line", "an open output stream")),
        Some(value) => Err(type_error(
            "fresh-line",
            "NIL, T, or an output stream",
            value,
        )),
    }
}

fn write_line(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("write-line", "1 to 2", arguments.len()));
    }
    let string = match &arguments[0] {
        Value::String(value) => value,
        value => return Err(type_error("write-line", "a string", value)),
    };
    let mut line = String::with_capacity(string.len() + 1);
    line.push_str(string);
    line.push('\n');
    write_destination("write-line", arguments.get(1), &line)?;
    Ok(arguments[0].clone())
}

fn close_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "close", 1)?;
    let stream = stream_reference("close", &arguments[0])?;
    stream.borrow_mut().close();
    Ok(Value::boolean(true))
}

fn streamp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "streamp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Stream(_))))
}

fn input_stream_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "input-stream-p", 1)?;
    let result = match &arguments[0] {
        Value::Stream(stream) => stream.borrow().is_input(),
        _ => false,
    };
    Ok(Value::boolean(result))
}

fn output_stream_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "output-stream-p", 1)?;
    let result = match &arguments[0] {
        Value::Stream(stream) => stream.borrow().is_output(),
        _ => false,
    };
    Ok(Value::boolean(result))
}

fn format_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity("format", "at least 2", arguments.len()));
    }
    let control = match &arguments[1] {
        Value::String(value) => value.as_ref(),
        value => return Err(type_error("format", "a string control", value)),
    };
    let output = format_control(control, &arguments[2..])?;
    match &arguments[0] {
        Value::Nil => Ok(Value::string(output)),
        Value::Boolean(true) => {
            print!("{output}");
            Ok(Value::Nil)
        }
        Value::Stream(stream) => {
            if stream.borrow_mut().write(&output) {
                Ok(Value::Nil)
            } else {
                Err(stream_state_error("format", "an open output stream"))
            }
        }
        value => Err(type_error("format", "NIL or T as the destination", value)),
    }
}

fn format_control(control: &str, arguments: &[Value]) -> Result<String, RuntimeError> {
    let characters = control.chars().collect::<Vec<_>>();
    let (output, _) = format_control_characters(&characters, arguments)?;
    Ok(output)
}

fn format_control_characters(
    characters: &[char],
    arguments: &[Value],
) -> Result<(String, usize), RuntimeError> {
    let mut output = String::new();
    let mut argument_index = 0;
    let mut character_index = 0;
    while character_index < characters.len() {
        let character = characters[character_index];
        character_index += 1;
        if character != '~' {
            output.push(character);
            continue;
        }

        let mut colon_modifier = false;
        let mut at_sign_modifier = false;
        while character_index < characters.len() {
            match characters[character_index] {
                ':' => {
                    colon_modifier = true;
                    character_index += 1;
                }
                '@' => {
                    at_sign_modifier = true;
                    character_index += 1;
                }
                _ => break,
            }
        }
        let directive =
            characters
                .get(character_index)
                .copied()
                .ok_or_else(|| RuntimeError::InvalidForm {
                    message: "format control ends after a tilde".to_string(),
                    span: None,
                })?;
        character_index += 1;
        let directive = directive.to_ascii_uppercase();
        if (colon_modifier || at_sign_modifier) && !matches!(directive, '{' | '[') {
            return Err(RuntimeError::InvalidForm {
                message: format!("unsupported format modifier before ~{directive}"),
                span: None,
            });
        }
        match directive {
            'A' => {
                let argument = format_argument("~A", arguments, &mut argument_index)?;
                append_aesthetic(&mut output, argument);
            }
            'S' => {
                let argument = format_argument("~S", arguments, &mut argument_index)?;
                output.push_str(&argument.to_string());
            }
            'D' | 'B' | 'O' | 'X' => {
                let argument =
                    format_argument("format integer directive", arguments, &mut argument_index)?;
                let integer = integer_argument("format", argument)?;
                let radix = match directive {
                    'D' => 10,
                    'B' => 2,
                    'O' => 8,
                    'X' => 16,
                    _ => unreachable!(),
                };
                output.push_str(&format_integer_radix(integer, radix));
            }
            'C' => {
                let argument = format_argument("~C", arguments, &mut argument_index)?;
                let Value::Character(character) = argument else {
                    return Err(type_error("format", "a character for ~C", argument));
                };
                output.push(*character);
            }
            '%' => output.push('\n'),
            '&' => {
                if !output.is_empty() && !output.ends_with('\n') {
                    output.push('\n');
                }
            }
            '~' => output.push('~'),
            '*' => {
                format_argument("~*", arguments, &mut argument_index)?;
            }
            '?' => {
                let nested_control = format_argument("~?", arguments, &mut argument_index)?;
                let nested_control = match nested_control {
                    Value::String(value) => value,
                    value => return Err(type_error("format", "a string for ~?", value)),
                };
                let nested_arguments = format_argument("~?", arguments, &mut argument_index)?;
                let nested_arguments = nested_arguments.list_items().ok_or_else(|| {
                    type_error("format", "a list of arguments for ~?", nested_arguments)
                })?;
                output.push_str(&format_control(&nested_control, &nested_arguments)?);
            }
            '{' => {
                let body_end = format_iteration_end(characters, character_index)?;
                let body = &characters[character_index..body_end];
                character_index = body_end + 2;
                if at_sign_modifier {
                    let (formatted, consumed) =
                        format_iteration(body, &arguments[argument_index..], colon_modifier)?;
                    output.push_str(&formatted);
                    argument_index += consumed;
                } else {
                    let list = format_argument("~{", arguments, &mut argument_index)?;
                    let list = list
                        .list_items()
                        .ok_or_else(|| type_error("format", "a list for ~{", list))?;
                    let (formatted, _) = format_iteration(body, &list, colon_modifier)?;
                    output.push_str(&formatted);
                }
            }
            '[' => {
                let body_end = format_choice_end(characters, character_index)?;
                let body = &characters[character_index..body_end];
                character_index = body_end + 2;
                let clauses = format_choice_clauses(body)?;
                if colon_modifier && at_sign_modifier {
                    return Err(RuntimeError::InvalidForm {
                        message: "format choice cannot use both : and @ modifiers".to_string(),
                        span: None,
                    });
                }
                if colon_modifier && clauses.len() != 2 {
                    return Err(RuntimeError::InvalidForm {
                        message: "boolean format choice needs two clauses".to_string(),
                        span: None,
                    });
                }
                if at_sign_modifier && clauses.len() != 1 {
                    return Err(RuntimeError::InvalidForm {
                        message: "at-sign format choice needs one clause".to_string(),
                        span: None,
                    });
                }
                let selector = format_argument("~[", arguments, &mut argument_index)?;
                let selected_index = if colon_modifier {
                    Some(usize::from(selector.is_truthy()))
                } else if at_sign_modifier {
                    selector.is_truthy().then_some(0)
                } else {
                    let index = integer_argument("format choice", selector)?;
                    usize::try_from(index).ok()
                };
                let selected_clause = selected_index.and_then(|index| {
                    clauses
                        .get(index)
                        .or_else(|| clauses.iter().find(|(_, default)| *default))
                });
                if let Some((clause, _)) = selected_clause {
                    let (formatted, consumed) =
                        format_control_characters(clause, &arguments[argument_index..])?;
                    output.push_str(&formatted);
                    argument_index += consumed;
                } else if !colon_modifier && !at_sign_modifier {
                    if let Some((clause, _)) = clauses.iter().find(|(_, default)| *default) {
                        let (formatted, consumed) =
                            format_control_characters(clause, &arguments[argument_index..])?;
                        output.push_str(&formatted);
                        argument_index += consumed;
                    }
                }
            }
            '}' => {
                return Err(RuntimeError::InvalidForm {
                    message: "unexpected format iteration terminator ~}".to_string(),
                    span: None,
                });
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("unsupported format directive ~{directive}"),
                    span: None,
                });
            }
        }
    }
    Ok((output, argument_index))
}

fn format_iteration_end(characters: &[char], start: usize) -> Result<usize, RuntimeError> {
    format_directive_end(characters, start, '{', "format iteration is missing ~}")
}

fn format_choice_end(characters: &[char], start: usize) -> Result<usize, RuntimeError> {
    format_directive_end(characters, start, '[', "format choice is missing ~]")
}

fn format_directive_end(
    characters: &[char],
    start: usize,
    opening: char,
    missing_message: &str,
) -> Result<usize, RuntimeError> {
    let mut stack = vec![opening];
    let mut index = start;
    while index < characters.len() {
        if characters[index] != '~' {
            index += 1;
            continue;
        }

        let mut directive_index = index + 1;
        while directive_index < characters.len() && matches!(characters[directive_index], ':' | '@')
        {
            directive_index += 1;
        }
        let Some(directive) = characters.get(directive_index).copied() else {
            break;
        };
        match directive.to_ascii_uppercase() {
            '{' | '[' => stack.push(directive.to_ascii_uppercase()),
            '}' | ']' => {
                let expected_opening = if directive == '}' { '{' } else { '[' };
                if stack.last().copied() == Some(expected_opening) {
                    stack.pop();
                    if stack.is_empty() {
                        return Ok(index);
                    }
                }
            }
            _ => {}
        }
        index = directive_index + 1;
    }
    Err(RuntimeError::InvalidForm {
        message: missing_message.to_string(),
        span: None,
    })
}

fn format_choice_clauses<'a>(body: &'a [char]) -> Result<Vec<(&'a [char], bool)>, RuntimeError> {
    let mut clauses = Vec::new();
    let mut clause_start = 0;
    let mut default_clause = false;
    let mut stack = Vec::new();
    let mut index = 0;
    while index < body.len() {
        if body[index] != '~' {
            index += 1;
            continue;
        }

        let mut directive_index = index + 1;
        let mut colon_modifier = false;
        let mut at_sign_modifier = false;
        while directive_index < body.len() {
            match body[directive_index] {
                ':' => {
                    colon_modifier = true;
                    directive_index += 1;
                }
                '@' => {
                    at_sign_modifier = true;
                    directive_index += 1;
                }
                _ => break,
            }
        }
        let Some(directive) = body.get(directive_index).copied() else {
            return Err(RuntimeError::InvalidForm {
                message: "format choice clause ends after a tilde".to_string(),
                span: None,
            });
        };
        let directive = directive.to_ascii_uppercase();
        match directive {
            '{' | '[' => stack.push(directive),
            '}' | ']' => {
                let expected_opening = if directive == '}' { '{' } else { '[' };
                if stack.last().copied() == Some(expected_opening) {
                    stack.pop();
                } else if stack.is_empty() {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unexpected format choice terminator ~{directive}"),
                        span: None,
                    });
                }
            }
            ';' if stack.is_empty() => {
                if at_sign_modifier {
                    return Err(RuntimeError::InvalidForm {
                        message: "at-sign modifier is not supported on a format choice clause"
                            .to_string(),
                        span: None,
                    });
                }
                clauses.push((&body[clause_start..index], default_clause));
                clause_start = directive_index + 1;
                default_clause = colon_modifier;
            }
            _ => {}
        }
        index = directive_index + 1;
    }
    if !stack.is_empty() {
        return Err(RuntimeError::InvalidForm {
            message: "format choice contains an unclosed nested directive".to_string(),
            span: None,
        });
    }
    clauses.push((&body[clause_start..], default_clause));
    Ok(clauses)
}

fn format_iteration(
    body: &[char],
    arguments: &[Value],
    colon_modifier: bool,
) -> Result<(String, usize), RuntimeError> {
    let mut output = String::new();
    let mut argument_index = 0;
    while argument_index < arguments.len() {
        let consumed = if colon_modifier {
            let nested_arguments = arguments[argument_index].list_items().ok_or_else(|| {
                type_error(
                    "format",
                    "a list element for ~:{",
                    &arguments[argument_index],
                )
            })?;
            let (formatted, consumed) = format_control_characters(body, &nested_arguments)?;
            output.push_str(&formatted);
            consumed
        } else {
            let (formatted, consumed) =
                format_control_characters(body, &arguments[argument_index..])?;
            output.push_str(&formatted);
            consumed
        };
        argument_index += if colon_modifier { 1 } else { consumed.max(1) };
    }
    Ok((output, argument_index))
}

fn format_argument<'a>(
    directive: &str,
    arguments: &'a [Value],
    argument_index: &mut usize,
) -> Result<&'a Value, RuntimeError> {
    let argument = arguments
        .get(*argument_index)
        .ok_or_else(|| RuntimeError::InvalidForm {
            message: format!("format directive {directive} needs another argument"),
            span: None,
        })?;
    *argument_index += 1;
    Ok(argument)
}

fn append_aesthetic(output: &mut String, value: &Value) {
    match value {
        Value::String(value) => output.push_str(value),
        Value::Character(value) => output.push(*value),
        Value::List(values) => {
            output.push('(');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(' ');
                }
                append_aesthetic(output, value);
            }
            output.push(')');
        }
        Value::DottedList { items, tail } => {
            output.push('(');
            for (index, value) in items.iter().enumerate() {
                if index != 0 {
                    output.push(' ');
                }
                append_aesthetic(output, value);
            }
            if !items.is_empty() {
                output.push(' ');
            }
            output.push_str(". ");
            append_aesthetic(output, tail);
            output.push(')');
        }
        Value::Vector(values) => {
            output.push_str("#(");
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(' ');
                }
                append_aesthetic(output, value);
            }
            output.push(')');
        }
        _ => output.push_str(&value.to_string()),
    }
}

fn format_integer_radix(value: i64, radix: u32) -> String {
    const DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    if value == 0 {
        return "0".to_string();
    }
    let negative = value < 0;
    let mut magnitude = value.unsigned_abs();
    let mut digits = Vec::new();
    while magnitude != 0 {
        digits.push(DIGITS[(magnitude % u64::from(radix)) as usize] as char);
        magnitude /= u64::from(radix);
    }
    if negative {
        digits.push('-');
    }
    digits.iter().rev().collect()
}

#[derive(Clone, Copy)]
enum Number {
    Integer(i64),
    Rational(Rational),
    Float(f64),
}

impl Number {
    fn as_float(self) -> f64 {
        match self {
            Self::Integer(value) => value as f64,
            Self::Rational(value) => value.numerator() as f64 / value.denominator() as f64,
            Self::Float(value) => value,
        }
    }

    fn is_float(&self) -> bool {
        matches!(self, Self::Float(_))
    }

    fn exact_parts(self) -> Option<(i64, i64)> {
        match self {
            Self::Integer(value) => Some((value, 1)),
            Self::Rational(value) => Some((value.numerator(), value.denominator())),
            Self::Float(_) => None,
        }
    }
}

impl Value {
    fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }
}

fn number(value: &Value) -> Result<Number, RuntimeError> {
    match value {
        Value::Integer(value) => Ok(Number::Integer(*value)),
        Value::Rational(value) => Ok(Number::Rational(*value)),
        Value::Float(value) => Ok(Number::Float(*value)),
        value => Err(number_error("numeric operation", value)),
    }
}

fn number_argument(function: &str, value: &Value) -> Result<Number, RuntimeError> {
    match value {
        Value::Integer(value) => Ok(Number::Integer(*value)),
        Value::Rational(value) => Ok(Number::Rational(*value)),
        Value::Float(value) => Ok(Number::Float(*value)),
        value => Err(number_error(function, value)),
    }
}

fn number_to_value(number: Number) -> Result<Value, RuntimeError> {
    match number {
        Number::Integer(value) => Ok(Value::Integer(value)),
        Number::Rational(value) => Value::rational(
            i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => Ok(Value::Float(value)),
    }
}

fn rational_number(numerator: i128, denominator: i128) -> Result<Number, RuntimeError> {
    let value = Rational::new(numerator, denominator)?;
    if value.denominator() == 1 {
        Ok(Number::Integer(value.numerator()))
    } else {
        Ok(Number::Rational(value))
    }
}

fn exact_binary(left: Number, right: Number, operation: char) -> Result<Number, RuntimeError> {
    let (left_numerator, left_denominator) = left
        .exact_parts()
        .expect("exact numeric operation received a float");
    let (right_numerator, right_denominator) = right
        .exact_parts()
        .expect("exact numeric operation received a float");
    let left_numerator = i128::from(left_numerator);
    let left_denominator = i128::from(left_denominator);
    let right_numerator = i128::from(right_numerator);
    let right_denominator = i128::from(right_denominator);
    let (numerator, denominator) = match operation {
        '+' => (
            left_numerator * right_denominator + right_numerator * left_denominator,
            left_denominator * right_denominator,
        ),
        '-' => (
            left_numerator * right_denominator - right_numerator * left_denominator,
            left_denominator * right_denominator,
        ),
        '*' => (
            left_numerator * right_numerator,
            left_denominator * right_denominator,
        ),
        '/' => (
            left_numerator * right_denominator,
            left_denominator * right_numerator,
        ),
        _ => unreachable!("unsupported exact numeric operation"),
    };
    rational_number(numerator, denominator)
}

fn negate_number(value: Number) -> Result<Number, RuntimeError> {
    match value {
        Number::Integer(value) => value
            .checked_neg()
            .map(Number::Integer)
            .ok_or(RuntimeError::NumericOverflow),
        Number::Rational(value) => rational_number(
            -i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => Ok(Number::Float(-value)),
    }
}

fn compare_number_values(left: Number, right: Number) -> Ordering {
    if left.is_float() || right.is_float() {
        return left
            .as_float()
            .partial_cmp(&right.as_float())
            .unwrap_or(Ordering::Equal);
    }
    let (left_numerator, left_denominator) = left
        .exact_parts()
        .expect("exact numeric comparison received a float");
    let (right_numerator, right_denominator) = right
        .exact_parts()
        .expect("exact numeric comparison received a float");
    (i128::from(left_numerator) * i128::from(right_denominator)).cmp(&(
        i128::from(right_numerator) * i128::from(left_denominator)
    ))
}

fn numeric_equalp(left: Number, right: Number) -> bool {
    compare_number_values(left, right) == Ordering::Equal
}

fn integer_argument(function: &str, value: &Value) -> Result<i64, RuntimeError> {
    value
        .as_integer()
        .ok_or_else(|| type_error(function, "integer", value))
}

fn number_error(function: &str, value: &Value) -> RuntimeError {
    type_error(function, "number", value)
}

fn exact(arguments: &[Value], function: &str, expected: usize) -> Result<(), RuntimeError> {
    if arguments.len() == expected {
        Ok(())
    } else {
        Err(arity(function, expected.to_string(), arguments.len()))
    }
}

fn arity(function: &str, expected: impl Into<String>, actual: usize) -> RuntimeError {
    RuntimeError::Arity {
        function: function.to_string(),
        expected: expected.into(),
        actual,
    }
}

fn type_error(function: &str, expected: &str, value: &Value) -> RuntimeError {
    RuntimeError::Type {
        expected: format!("{function} requires {expected}"),
        actual: value.type_name().to_string(),
        span: None,
    }
}

macro_rules! numeric_support_builtins {
    () => {
#[derive(Clone, Copy)]
enum Number {
    Integer(i64),
    Rational(Rational),
    Float(f64),
}

#[derive(Clone, Copy)]
enum Numeric {
    Real(Number),
    Complex { real: Number, imag: Number },
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

impl Numeric {
    fn into_complex(self) -> (Number, Number) {
        match self {
            Self::Real(value) => (value, Number::Integer(0)),
            Self::Complex { real, imag } => (real, imag),
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

fn numeric_argument(function: &str, value: &Value) -> Result<Numeric, RuntimeError> {
    match value {
        Value::Complex { real, imag } => Ok(Numeric::Complex {
            real: number_argument(function, real.as_ref())?,
            imag: number_argument(function, imag.as_ref())?,
        }),
        _ => Ok(Numeric::Real(number_argument(function, value)?)),
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

fn numeric_to_value(number: Numeric) -> Result<Value, RuntimeError> {
    match number {
        Numeric::Real(value) => number_to_value(value),
        Numeric::Complex { real, imag } => Ok(Value::complex(
            number_to_value(real)?,
            number_to_value(imag)?,
        )),
    }
}

fn square_root_numeric(number: Numeric) -> Result<Numeric, RuntimeError> {
    let value = match number {
        Numeric::Real(number) => square_root_real(number)?,
        Numeric::Complex { real, imag } => square_root_complex(real, imag)?,
    };

    numeric_argument("sqrt", &value)
}

fn canonicalize_number(number: Number) -> Number {
    match number {
        Number::Float(value) => canonicalize_float(value),
        value => value,
    }
}

fn canonicalize_numeric(number: Numeric) -> Numeric {
    match number {
        Numeric::Real(value) => Numeric::Real(canonicalize_number(value)),
        Numeric::Complex { real, imag } => {
            let real = canonicalize_number(real);
            let imag = canonicalize_number(imag);
            if imag.as_float() == 0.0 {
                Numeric::Real(real)
            } else {
                Numeric::Complex { real, imag }
            }
        }
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

fn add_numbers(left: Number, right: Number) -> Result<Number, RuntimeError> {
    if left.is_float() || right.is_float() {
        Ok(Number::Float(left.as_float() + right.as_float()))
    } else {
        exact_binary(left, right, '+')
    }
}

fn subtract_numbers(left: Number, right: Number) -> Result<Number, RuntimeError> {
    if left.is_float() || right.is_float() {
        Ok(Number::Float(left.as_float() - right.as_float()))
    } else {
        exact_binary(left, right, '-')
    }
}

fn multiply_numbers(left: Number, right: Number) -> Result<Number, RuntimeError> {
    if left.is_float() || right.is_float() {
        Ok(Number::Float(left.as_float() * right.as_float()))
    } else {
        exact_binary(left, right, '*')
    }
}

fn divide_numbers(left: Number, right: Number) -> Result<Number, RuntimeError> {
    if right.as_float() == 0.0 {
        return Err(RuntimeError::DivisionByZero);
    }
    if left.is_float() || right.is_float() {
        Ok(Number::Float(left.as_float() / right.as_float()))
    } else {
        exact_binary(left, right, '/')
    }
}

fn negate_numeric(value: Numeric) -> Result<Numeric, RuntimeError> {
    match value {
        Numeric::Real(value) => Ok(Numeric::Real(negate_number(value)?)),
        Numeric::Complex { real, imag } => Ok(Numeric::Complex {
            real: negate_number(real)?,
            imag: negate_number(imag)?,
        }),
    }
}

fn add_numeric(left: Numeric, right: Numeric) -> Result<Numeric, RuntimeError> {
    match (left, right) {
        (Numeric::Real(left), Numeric::Real(right)) => Ok(Numeric::Real(add_numbers(left, right)?)),
        (left, right) => {
            let (left_real, left_imag) = left.into_complex();
            let (right_real, right_imag) = right.into_complex();
            Ok(Numeric::Complex {
                real: add_numbers(left_real, right_real)?,
                imag: add_numbers(left_imag, right_imag)?,
            })
        }
    }
}

fn subtract_numeric(left: Numeric, right: Numeric) -> Result<Numeric, RuntimeError> {
    match (left, right) {
        (Numeric::Real(left), Numeric::Real(right)) => {
            Ok(Numeric::Real(subtract_numbers(left, right)?))
        }
        (left, right) => {
            let (left_real, left_imag) = left.into_complex();
            let (right_real, right_imag) = right.into_complex();
            Ok(Numeric::Complex {
                real: subtract_numbers(left_real, right_real)?,
                imag: subtract_numbers(left_imag, right_imag)?,
            })
        }
    }
}

fn multiply_numeric(left: Numeric, right: Numeric) -> Result<Numeric, RuntimeError> {
    match (left, right) {
        (Numeric::Real(left), Numeric::Real(right)) => {
            Ok(Numeric::Real(multiply_numbers(left, right)?))
        }
        (left, right) => {
            let (left_real, left_imag) = left.into_complex();
            let (right_real, right_imag) = right.into_complex();
            let ac = multiply_numbers(left_real, right_real)?;
            let bd = multiply_numbers(left_imag, right_imag)?;
            let ad = multiply_numbers(left_real, right_imag)?;
            let bc = multiply_numbers(left_imag, right_real)?;
            Ok(Numeric::Complex {
                real: subtract_numbers(ac, bd)?,
                imag: add_numbers(ad, bc)?,
            })
        }
    }
}

fn divide_numeric(left: Numeric, right: Numeric) -> Result<Numeric, RuntimeError> {
    match (left, right) {
        (Numeric::Real(left), Numeric::Real(right)) => {
            Ok(Numeric::Real(divide_numbers(left, right)?))
        }
        (left, right) => {
            let (left_real, left_imag) = left.into_complex();
            let (right_real, right_imag) = right.into_complex();
            let denominator = add_numbers(
                multiply_numbers(right_real, right_real)?,
                multiply_numbers(right_imag, right_imag)?,
            )?;
            let real = add_numbers(
                multiply_numbers(left_real, right_real)?,
                multiply_numbers(left_imag, right_imag)?,
            )?;
            let imag = subtract_numbers(
                multiply_numbers(left_imag, right_real)?,
                multiply_numbers(left_real, right_imag)?,
            )?;
            Ok(Numeric::Complex {
                real: divide_numbers(real, denominator)?,
                imag: divide_numbers(imag, denominator)?,
            })
        }
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
    (i128::from(left_numerator) * i128::from(right_denominator))
        .cmp(&(i128::from(right_numerator) * i128::from(left_denominator)))
}

fn numeric_equalp(left: Number, right: Number) -> bool {
    compare_number_values(left, right) == Ordering::Equal
}

fn numeric_equal_values(left: Numeric, right: Numeric) -> bool {
    match (left, right) {
        (Numeric::Real(left), Numeric::Real(right)) => numeric_equalp(left, right),
        (left, right) => {
            let (left_real, left_imag) = left.into_complex();
            let (right_real, right_imag) = right.into_complex();
            numeric_equalp(left_real, right_real) && numeric_equalp(left_imag, right_imag)
        }
    }
}

fn byte_spec_value(size: i64, position: i64) -> Value {
    Value::list(vec![
        Value::symbol("BYTE"),
        Value::Integer(size),
        Value::Integer(position),
    ])
}

pub(crate) fn parse_byte_spec(function: &str, value: &Value) -> Result<(u32, u32), RuntimeError> {
    let Some(items) = value.list_items() else {
        return Err(type_error(function, "a byte specifier", value));
    };
    let [operator, size, position] = items.as_slice() else {
        return Err(type_error(function, "a byte specifier", value));
    };
    if operator
        .symbol_name()
        .map(package::normalize_symbol_name)
        .as_deref()
        != Some("BYTE")
    {
        return Err(type_error(function, "a byte specifier", value));
    }
    let size = integer_argument(function, size)?;
    let position = integer_argument(function, position)?;
    validate_byte_bounds(function, size, position)?;
    Ok((size as u32, position as u32))
}

fn validate_byte_bounds(function: &str, size: i64, position: i64) -> Result<(), RuntimeError> {
    if size < 0 {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} byte size must be non-negative, got {size}"),
            span: None,
        });
    }
    if position < 0 {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} byte position must be non-negative, got {position}"),
            span: None,
        });
    }
    if position >= 64 {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} byte position must be less than 64, got {position}"),
            span: None,
        });
    }
    if size > 64 - position {
        return Err(RuntimeError::InvalidForm {
            message: format!(
                "{function} byte size {size} at position {position} exceeds the 64-bit integer range"
            ),
            span: None,
        });
    }
    Ok(())
}

fn validate_bit_index(function: &str, index: i64) -> Result<(), RuntimeError> {
    if index < 0 {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} bit index must be non-negative, got {index}"),
            span: None,
        });
    }
    if index >= 64 {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} bit index must be less than 64, got {index}"),
            span: None,
        });
    }
    Ok(())
}

fn byte_mask(size: u32, position: u32) -> u64 {
    if size == 0 {
        0
    } else {
        (u64::MAX >> (64 - size)) << position
    }
}

fn extract_byte_field(integer: u64, size: u32, position: u32) -> u64 {
    if size == 0 {
        0
    } else {
        (integer >> position) & (u64::MAX >> (64 - size))
    }
}

fn integer_argument(function: &str, value: &Value) -> Result<i64, RuntimeError> {
    value
        .as_integer()
        .ok_or_else(|| type_error(function, "integer", value))
}

fn is_real_number(value: &Value) -> bool {
    matches!(
        value,
        Value::Integer(_) | Value::Rational(_) | Value::Float(_)
    )
}

fn real_number_argument(function: &str, value: &Value) -> Result<Value, RuntimeError> {
    if is_real_number(value) {
        Ok(value.clone())
    } else {
        Err(type_error(function, "real number", value))
    }
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

    };
}

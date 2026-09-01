#[allow(clippy::wildcard_imports)]
use super::*;

pub fn execute_character_unary_instruction(
    operation: &str,
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    let value = stack
        .pop()
        .ok_or_else(|| invalid("unary character operation has too few stack values", span))?;
    let result = match operation {
        "CHARACTER" => crate::builtins::character_value(&[value]),
        "CHAR-CODE" => crate::builtins::char_code(&[value]),
        "CHAR-INT" => crate::builtins::char_int(&[value]),
        "CODE-CHAR" => crate::builtins::code_char(&[value]),
        "INT-CHAR" => crate::builtins::int_char(&[value]),
        "CHAR-UPCASE" => crate::builtins::character_upcase(&[value]),
        "CHAR-DOWNCASE" => crate::builtins::character_downcase(&[value]),
        "CHAR-NAME" => crate::builtins::character_name(&[value]),
        "NAME-CHAR" => crate::builtins::name_character(&[value]),
        _ => Err(invalid("unknown unary character operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_character_comparison_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid(
            "character comparison has too few stack values",
            span,
        ));
    }
    let start = stack.len() - argument_count;
    let arguments = stack
        .drain(start..)
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let result = match operation {
        "CHAR=" => crate::builtins::character_equal(&arguments),
        "CHAR/=" => crate::builtins::character_not_equal(&arguments),
        "CHAR-EQUAL" => crate::builtins::character_case_equal(&arguments),
        "CHAR-NOT-EQUAL" => crate::builtins::character_case_not_equal(&arguments),
        "CHAR<" => crate::builtins::character_less_than(&arguments),
        "CHAR>" => crate::builtins::character_greater_than(&arguments),
        "CHAR<=" => crate::builtins::character_less_equal(&arguments),
        "CHAR>=" => crate::builtins::character_greater_equal(&arguments),
        "CHAR-LESSP" => crate::builtins::character_case_less_than(&arguments),
        "CHAR-GREATERP" => crate::builtins::character_case_greater_than(&arguments),
        "CHAR-NOT-LESSP" => crate::builtins::character_case_greater_equal(&arguments),
        "CHAR-NOT-GREATERP" => crate::builtins::character_case_less_equal(&arguments),
        _ => Err(invalid("unknown character comparison operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_symbol_unary_instruction(
    operation: &str,
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    let value = stack
        .pop()
        .ok_or_else(|| invalid("unary symbol operation has too few stack values", span))?;
    let result = match operation {
        "SYMBOL-NAME" => crate::builtins::symbol_name_value(&[value]),
        "SYMBOL-PACKAGE" => crate::builtins::symbol_package_value(&[value]),
        _ => Err(invalid("unknown unary symbol operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_value_unary_instruction(
    operation: &str,
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    let value = stack
        .pop()
        .ok_or_else(|| invalid("unary value operation has too few stack values", span))?;
    let result = match operation {
        "IDENTITY" => crate::builtins::identity(&[value]),
        "TYPE-OF" => crate::builtins::type_of(&[value]),
        _ => Err(invalid("unknown unary value operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_type_predicate_instruction(
    operation: &str,
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    let value = stack
        .pop()
        .ok_or_else(|| invalid("type predicate has too few stack values", span))?;
    let result = match operation {
        "ATOM" => crate::builtins::atom(&[value]),
        "CONSP" => crate::builtins::consp(&[value]),
        "LISTP" => crate::builtins::listp(&[value]),
        "NUMBERP" => crate::builtins::numberp(&[value]),
        "COMPLEXP" => crate::builtins::complexp(&[value]),
        "INTEGERP" => crate::builtins::integerp(&[value]),
        "FLOATP" => crate::builtins::floatp(&[value]),
        "RATIONALP" => crate::builtins::rationalp(&[value]),
        "STRINGP" => crate::builtins::stringp(&[value]),
        "SIMPLE-STRING-P" => crate::builtins::simple_string_p(&[value]),
        "CHARACTERP" => crate::builtins::characterp(&[value]),
        "SYMBOLP" => crate::builtins::symbolp(&[value]),
        "PACKAGEP" => crate::builtins::packagep(&[value]),
        "KEYWORDP" => crate::builtins::keywordp(&[value]),
        "VECTORP" => crate::builtins::vectorp(&[value]),
        "FUNCTIONP" => crate::builtins::functionp(&[value]),
        "SIMPLE-VECTOR-P" => crate::builtins::simple_vector_p(&[value]),
        "BIT-VECTOR-P" => crate::builtins::bit_vector_p(&[value]),
        "SIMPLE-BIT-VECTOR-P" => crate::builtins::simple_bit_vector_p(&[value]),
        "ARRAYP" => crate::builtins::arrayp(&[value]),
        "SIMPLE-ARRAY-P" => crate::builtins::simple_array_p(&[value]),
        "HASH-TABLE-P" => crate::builtins::hash_table_p(&[value]),
        "RANDOM-STATE-P" => crate::builtins::random_state_p(&[value]),
        "ALPHA-CHAR-P" => crate::builtins::alpha_character_p(&[value]),
        "ALPHANUMERICP" => crate::builtins::alphanumeric_p(&[value]),
        "GRAPHIC-CHAR-P" => crate::builtins::graphic_character_p(&[value]),
        "STANDARD-CHAR-P" => crate::builtins::standard_character_p(&[value]),
        "UPPER-CASE-P" => crate::builtins::upper_case_p(&[value]),
        "LOWER-CASE-P" => crate::builtins::lower_case_p(&[value]),
        "BOTH-CASE-P" => crate::builtins::both_case_p(&[value]),
        "DIGIT-CHAR-P" => crate::builtins::digit_character_p(&[value]),
        "STREAMP" => crate::builtins::streamp(&[value]),
        "INPUT-STREAM-P" => crate::builtins::input_stream_p(&[value]),
        "OUTPUT-STREAM-P" => crate::builtins::output_stream_p(&[value]),
        "OPEN-STREAM-P" => crate::builtins::open_stream_p(&[value]),
        "NOT" | "NULL" => Ok(Value::boolean(!value.is_truthy())),
        _ => Err(invalid("unknown type predicate operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_typep_instruction(
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < 2 {
        return Err(invalid("typep has too few stack values", span));
    }
    let type_designator = stack.pop().expect("checked stack length");
    let value = stack.pop().expect("checked stack length");
    stack.push(crate::builtins::typep(&[value, type_designator])?);
    Ok(())
}

pub fn execute_character_predicate_instruction(
    operation: &str,
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    let value = stack
        .pop()
        .ok_or_else(|| invalid("character predicate has too few stack values", span))?;
    let result = match operation {
        "ALPHA-CHAR-P" => crate::builtins::alpha_character_p(&[value]),
        "ALPHANUMERICP" => crate::builtins::alphanumeric_p(&[value]),
        "GRAPHIC-CHAR-P" => crate::builtins::graphic_character_p(&[value]),
        "STANDARD-CHAR-P" => crate::builtins::standard_character_p(&[value]),
        "UPPER-CASE-P" => crate::builtins::upper_case_p(&[value]),
        "LOWER-CASE-P" => crate::builtins::lower_case_p(&[value]),
        "BOTH-CASE-P" => crate::builtins::both_case_p(&[value]),
        _ => Err(invalid("unknown character predicate operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_equality_instruction(
    operation: &str,
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    let right = stack
        .pop()
        .ok_or_else(|| invalid("equality predicate has too few stack values", span))?;
    let left = stack
        .pop()
        .ok_or_else(|| invalid("equality predicate has too few stack values", span))?;
    let result = match operation {
        "EQ" => left.eq_value(&right),
        "EQL" => crate::builtins::eql_value(&left, &right),
        "EQUAL" => left.equal_value(&right),
        "EQUALP" => crate::builtins::equalp_value(&left, &right),
        _ => return Err(invalid("unknown equality predicate operation", span)),
    };
    stack.push(Value::boolean(result));
    Ok(())
}

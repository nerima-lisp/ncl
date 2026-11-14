//! Typed failures from the Common Lisp number context.

use ncl_object::typed::ConditionErrorData;
use ncl_object::{ArithmeticError, LispError, Word};

/// An arithmetic failure with its standard condition data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum NumberError {
    /// A division operation received a zero divisor.
    DivisionByZero {
        /// The Lisp operation designator.
        operation: Word,
        /// The Lisp operand list.
        operands: Word,
    },
    /// An arithmetic operation could not be completed.
    InvalidOperation {
        /// The Lisp operation designator.
        operation: Word,
        /// The Lisp operand list.
        operands: Word,
    },
}

impl std::fmt::Display for NumberError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DivisionByZero { .. } => formatter.write_str("division by zero"),
            Self::InvalidOperation { .. } => formatter.write_str("invalid arithmetic operation"),
        }
    }
}

impl std::error::Error for NumberError {}

impl From<NumberError> for LispError {
    fn from(error: NumberError) -> Self {
        let data = match error {
            NumberError::DivisionByZero {
                operation,
                operands,
            } => ConditionErrorData::ArithmeticError {
                kind: ArithmeticError::DivisionByZero,
                operation,
                operands,
            },
            NumberError::InvalidOperation {
                operation,
                operands,
            } => ConditionErrorData::ArithmeticError {
                kind: ArithmeticError::InvalidOperation,
                operation,
                operands,
            },
        };
        Self::Condition(data)
    }
}

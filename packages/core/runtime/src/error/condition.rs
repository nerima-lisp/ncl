use std::borrow::Cow;

use crate::environment::{intern_name, names_equal};
use crate::error::{ConditionName, RuntimeError};

impl RuntimeError {
    pub(crate) fn condition_type_name(&self) -> Cow<'_, str> {
        match self {
            Self::Read(_) => Cow::Borrowed("READER-ERROR"),
            Self::Compile(_) => Cow::Borrowed("COMPILER-ERROR"),
            Self::UnboundVariable { .. } => Cow::Borrowed("UNBOUND-VARIABLE"),
            Self::UnboundSlot { .. } => Cow::Borrowed("UNBOUND-SLOT"),
            Self::NotCallable { .. } | Self::Type { .. } => Cow::Borrowed("TYPE-ERROR"),
            Self::Arity { .. } => Cow::Borrowed("PROGRAM-ERROR"),
            Self::InvalidForm { .. } => Cow::Borrowed("SIMPLE-ERROR"),
            Self::Signaled(error) => {
                if error.warning {
                    Cow::Borrowed("SIMPLE-WARNING")
                } else {
                    Cow::Borrowed(error.condition.as_ref())
                }
            }
            Self::Package { .. } => Cow::Borrowed("PACKAGE-ERROR"),
            Self::ReturnFrom { .. }
            | Self::Go { .. }
            | Self::Throw { .. }
            | Self::InvokeRestart { .. } => Cow::Borrowed("CONTROL-ERROR"),
            Self::DivisionByZero => Cow::Borrowed("DIVISION-BY-ZERO"),
            Self::NumericOverflow => Cow::Borrowed("ARITHMETIC-ERROR"),
            Self::Io { .. } => Cow::Borrowed("FILE-ERROR"),
        }
    }

    pub(crate) fn matches_condition(&self, condition: &str) -> bool {
        if matches!(
            self,
            Self::ReturnFrom { .. }
                | Self::Go { .. }
                | Self::Throw { .. }
                | Self::InvokeRestart { .. }
        ) {
            return false;
        }

        let condition = condition.trim_start_matches(':');
        if names_equal(condition, "CONDITION")
            || names_equal(condition, "ERROR")
            || names_equal(condition, "SERIOUS-CONDITION")
        {
            return match self {
                Self::Signaled(error) => {
                    if names_equal(condition, "CONDITION") {
                        true
                    } else if error.warning {
                        false
                    } else {
                        // A condition's own name and every entry in its
                        // condition_types (its full ancestor chain) can each
                        // independently place it in the ERROR hierarchy: an
                        // application-defined condition whose condition_types
                        // includes the built-in TYPE-ERROR is itself a
                        // type-error, even though its own name is not.
                        std::iter::once(error.condition.as_ref())
                            .chain(
                                error
                                    .condition_types
                                    .iter()
                                    .map(std::convert::AsRef::as_ref),
                            )
                            .any(|name| {
                                names_equal(name, condition)
                                    || matches!(
                                        name,
                                        "SIMPLE-ERROR"
                                            | "DIVISION-BY-ZERO"
                                            | "ARITHMETIC-ERROR"
                                            | "TYPE-ERROR"
                                            | "PROGRAM-ERROR"
                                            | "PACKAGE-ERROR"
                                            | "READER-ERROR"
                                            | "COMPILER-ERROR"
                                            | "FILE-ERROR"
                                            | "UNBOUND-VARIABLE"
                                            | "UNBOUND-SLOT"
                                    )
                            })
                    }
                }
                _ => true,
            };
        }

        match self {
            Self::Signaled(error) => {
                names_equal(condition, error.condition.as_ref())
                    || error
                        .condition_types
                        .iter()
                        .any(|type_name| names_equal(type_name.as_ref(), condition))
                    || (error.warning && names_equal(condition, "WARNING"))
                    || (!error.warning && names_equal(condition, "SIMPLE-CONDITION"))
            }
            Self::DivisionByZero => {
                names_equal(condition, "DIVISION-BY-ZERO")
                    || names_equal(condition, "ARITHMETIC-ERROR")
            }
            Self::NumericOverflow => names_equal(condition, "ARITHMETIC-ERROR"),
            _ => names_equal(condition, self.condition_type_name().as_ref()),
        }
    }
}

/// Canonicalizes a condition-type name to the form [`SignaledError`]'s
/// `condition` and `condition_types` fields are stored in: uppercase, with
/// any leading keyword-package colon stripped. Callers that build a
/// [`SignaledError`] must normalize through this function so that
/// [`RuntimeError::matches_condition`] can compare stored names directly
/// without re-normalizing them on every lookup.
pub fn normalize_condition_name(condition: &str) -> ConditionName {
    intern_name(condition.trim_start_matches(':'))
}

#[cfg(test)]
mod tests;

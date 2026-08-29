use crate::error::RuntimeError;

impl RuntimeError {
    pub(crate) fn condition_type_name(&self) -> String {
        match self {
            Self::Read(_) => "READER-ERROR".to_owned(),
            Self::Compile(_) => "COMPILER-ERROR".to_owned(),
            Self::UnboundVariable { .. } => "UNBOUND-VARIABLE".to_owned(),
            Self::NotCallable { .. } | Self::Type { .. } => "TYPE-ERROR".to_owned(),
            Self::Arity { .. } => "PROGRAM-ERROR".to_owned(),
            Self::InvalidForm { .. } => "SIMPLE-ERROR".to_owned(),
            Self::Signaled(error) => {
                if error.warning {
                    "SIMPLE-WARNING".to_owned()
                } else {
                    error.condition.clone()
                }
            }
            Self::Package { .. } => "PACKAGE-ERROR".to_owned(),
            Self::ReturnFrom { .. }
            | Self::Go { .. }
            | Self::Throw { .. }
            | Self::InvokeRestart { .. } => "CONTROL-ERROR".to_owned(),
            Self::DivisionByZero => "DIVISION-BY-ZERO".to_owned(),
            Self::NumericOverflow => "ARITHMETIC-ERROR".to_owned(),
            Self::Io { .. } => "FILE-ERROR".to_owned(),
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

        let condition = normalize_condition_name(condition);
        if matches!(
            condition.as_str(),
            "CONDITION" | "ERROR" | "SERIOUS-CONDITION"
        ) {
            return match self {
                Self::Signaled(error) => {
                    if condition == "CONDITION" {
                        true
                    } else if error.warning {
                        false
                    } else {
                        error
                            .condition_types
                            .iter()
                            .any(|type_name| type_name.as_str() == condition)
                            || matches!(
                                error.condition.as_str(),
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
                            )
                    }
                }
                _ => true,
            };
        }

        match self {
            Self::Signaled(error) => {
                condition == error.condition
                    || error
                        .condition_types
                        .iter()
                        .any(|type_name| type_name.as_str() == condition)
                    || (error.warning && condition == "WARNING")
                    || (!error.warning && condition == "SIMPLE-CONDITION")
            }
            Self::DivisionByZero => {
                matches!(condition.as_str(), "DIVISION-BY-ZERO" | "ARITHMETIC-ERROR")
            }
            Self::NumericOverflow => condition == "ARITHMETIC-ERROR",
            _ => condition == self.condition_type_name(),
        }
    }
}

/// Canonicalizes a condition-type name to the form [`SignaledError`]'s
/// `condition` and `condition_types` fields are stored in: uppercase, with
/// any leading keyword-package colon stripped. Callers that build a
/// [`SignaledError`] must normalize through this function so that
/// [`RuntimeError::matches_condition`] can compare stored names directly
/// without re-normalizing them on every lookup.
pub fn normalize_condition_name(condition: &str) -> String {
    condition.trim_start_matches(':').to_ascii_uppercase()
}

#[cfg(test)]
mod tests;

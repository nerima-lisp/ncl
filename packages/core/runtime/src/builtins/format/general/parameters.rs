use crate::RuntimeError;
use crate::builtins::format::model::FormatParameter;
use crate::builtins::format::parameters::format_parameter_count;

pub(in crate::builtins::format) struct GeneralFloatParameters {
    pub(in crate::builtins::format) minimum_column: usize,
    pub(in crate::builtins::format) requested_fractional_digits: Option<usize>,
    pub(in crate::builtins::format) exponent_padding: usize,
    pub(in crate::builtins::format) exponent_character: FormatParameter,
}

pub(in crate::builtins::format) fn parse_general_float_parameters(
    parameters: &[FormatParameter],
) -> Result<GeneralFloatParameters, RuntimeError> {
    let parameter_at = |index| {
        parameters
            .get(index)
            .copied()
            .unwrap_or(FormatParameter::Missing)
    };
    let requested_fractional_digits = match parameter_at(1) {
        FormatParameter::Missing => None,
        FormatParameter::Number(value) => {
            Some(
                usize::try_from(value).map_err(|_| RuntimeError::InvalidForm {
                    message: "format fractional digit count must be non-negative".to_string(),
                    span: None,
                })?,
            )
        }
        FormatParameter::Character(_) => {
            return Err(RuntimeError::InvalidForm {
                message: "format parameter 1 must be numeric".to_string(),
                span: None,
            });
        }
    };
    let exponent_padding = match parameter_at(2) {
        FormatParameter::Missing => 4,
        FormatParameter::Number(value) => usize::try_from(value)
            .map_err(|_| RuntimeError::InvalidForm {
                message: "format exponent field count must be non-negative".to_string(),
                span: None,
            })?
            .checked_add(2)
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: "format exponent field count is out of range".to_string(),
                span: None,
            })?,
        FormatParameter::Character(_) => {
            return Err(RuntimeError::InvalidForm {
                message: "format parameter 2 must be numeric".to_string(),
                span: None,
            });
        }
    };
    Ok(GeneralFloatParameters {
        minimum_column: format_parameter_count(parameters, 0, 0)?,
        requested_fractional_digits,
        exponent_padding,
        exponent_character: match parameter_at(6) {
            FormatParameter::Missing => FormatParameter::Character('e'),
            parameter => parameter,
        },
    })
}

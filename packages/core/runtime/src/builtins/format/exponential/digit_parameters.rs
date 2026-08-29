use crate::RuntimeError;
use crate::builtins::format::model::FormatParameter;

pub(in crate::builtins::format) fn exponential_digit_parameters(
    parameters: &[FormatParameter],
) -> Result<(Option<usize>, Option<usize>), RuntimeError> {
    let parse = |index, kind| match parameters
        .get(index)
        .copied()
        .unwrap_or(FormatParameter::Missing)
    {
        FormatParameter::Missing => Ok(None),
        FormatParameter::Number(value) => {
            usize::try_from(value)
                .map(Some)
                .map_err(|_| RuntimeError::InvalidForm {
                    message: format!("format {kind} digit count must be non-negative"),
                    span: None,
                })
        }
        FormatParameter::Character(_) => Err(RuntimeError::InvalidForm {
            message: format!("format parameter {index} must be numeric"),
            span: None,
        }),
    };
    Ok((parse(1, "fractional")?, parse(2, "exponent")?))
}

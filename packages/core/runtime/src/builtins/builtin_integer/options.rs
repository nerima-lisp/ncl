use crate::builtins::{array_option_name, index_argument, integer_argument};
use crate::{RuntimeError, Value};

#[derive(Debug, Copy, Clone)]
pub(super) struct ParseIntegerOptions {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) radix: u32,
    pub(super) junk_allowed: bool,
}

impl ParseIntegerOptions {
    pub(super) fn from_arguments(
        arguments: &[Value],
        character_count: usize,
    ) -> Result<Self, RuntimeError> {
        let mut options = Self {
            start: 0,
            end: character_count,
            radix: 10,
            junk_allowed: false,
        };
        for pair in arguments.as_chunks::<2>().0 {
            match array_option_name("parse-integer", &pair[0])?.as_str() {
                "START" => options.start = index_argument("parse-integer", &pair[1])?,
                "END" => options.end = index_argument("parse-integer", &pair[1])?,
                "RADIX" => {
                    let radix = integer_argument("parse-integer", &pair[1])?;
                    options.radix = u32::try_from(radix).map_err(|_| invalid_radix(radix))?;
                }
                "JUNK-ALLOWED" => options.junk_allowed = pair[1].is_truthy(),
                option => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("parse-integer does not accept :{option}"),
                        span: None,
                    });
                }
            }
        }
        if options.start > options.end || options.end > character_count {
            return Err(RuntimeError::InvalidForm {
                message: "parse-integer bounds are invalid".to_string(),
                span: None,
            });
        }
        if !(2..=36).contains(&options.radix) {
            return Err(invalid_radix(i64::from(options.radix)));
        }
        Ok(options)
    }
}

fn invalid_radix(radix: i64) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: format!("parse-integer radix must be between 2 and 36, got {radix}"),
        span: None,
    }
}

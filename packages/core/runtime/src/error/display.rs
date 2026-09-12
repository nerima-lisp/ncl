use std::fmt;

use ncl_syntax::Span;

use crate::error::RuntimeError;

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => error.fmt(formatter),
            Self::Compile(error) => error.fmt(formatter),
            Self::UnboundVariable { name, span } => {
                write!(formatter, "unbound variable {name}")?;
                write_span(formatter, *span)
            }
            Self::UnboundSlot { name, span } => {
                write!(formatter, "slot {name} is unbound")?;
                write_span(formatter, *span)
            }
            Self::NotCallable { value, span } => {
                write!(formatter, "{value} is not callable")?;
                write_span(formatter, *span)
            }
            Self::Arity {
                function,
                expected,
                actual,
            } => write!(
                formatter,
                "{function} expected {expected} arguments, received {actual}"
            ),
            Self::Type {
                expected,
                actual,
                span,
            } => {
                write!(formatter, "expected {expected}, received {actual}")?;
                write_span(formatter, *span)
            }
            Self::InvalidForm { message, span } | Self::Package { message, span } => {
                formatter.write_str(message)?;
                write_span(formatter, *span)
            }
            Self::Signaled(error) => {
                formatter.write_str(&error.message)?;
                write_span(formatter, error.span)
            }
            Self::ReturnFrom { block, span, .. } => {
                write!(formatter, "return-from {block}")?;
                write_span(formatter, *span)
            }
            Self::Go { tag, span, .. } => {
                write!(formatter, "go {tag}")?;
                write_span(formatter, *span)
            }
            Self::Throw { tag, span, .. } => {
                write!(formatter, "throw {tag}")?;
                write_span(formatter, *span)
            }
            Self::InvokeRestart { name, span, .. } => {
                write!(formatter, "invoke-restart {name}")?;
                write_span(formatter, *span)
            }
            Self::DivisionByZero => formatter.write_str("division by zero"),
            Self::NumericOverflow => formatter.write_str("numeric overflow"),
            Self::Io { message, .. } => formatter.write_str(message),
        }
    }
}

fn write_span(formatter: &mut fmt::Formatter<'_>, span: Option<Span>) -> fmt::Result {
    if let Some(span) = span {
        write!(formatter, " at byte {}..{}", span.start, span.end)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;

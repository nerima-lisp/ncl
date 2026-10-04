//! Errors raised while rendering Lisp text.

use ncl_object::ObjectError;

/// A failure while rendering an object as Lisp text.
///
/// The printer never panics on a bad object; every accessor failure is
/// converted into [`PrintError::Object`].
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PrintError {
    /// An `ncl-object` accessor rejected the value.
    Object(ObjectError),
    /// The output sink refused a write.
    Sink(String),
    /// `*print-readably*` was true and the object has no readable syntax.
    NotReadable,
    /// A circular structure was reached while `*print-circle*` was false.
    Circularity,
}

impl std::fmt::Display for PrintError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Object(error) => write!(formatter, "print: object error: {error}"),
            Self::Sink(message) => write!(formatter, "print: sink error: {message}"),
            Self::NotReadable => write!(formatter, "print: object is not readable"),
            Self::Circularity => {
                write!(
                    formatter,
                    "print: circular structure without *print-circle*"
                )
            }
        }
    }
}

impl std::error::Error for PrintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Object(error) => Some(error),
            Self::Sink(_) | Self::NotReadable | Self::Circularity => None,
        }
    }
}

impl From<ObjectError> for PrintError {
    fn from(value: ObjectError) -> Self {
        Self::Object(value)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "coverage tests assert on setup and output"
)]
mod tests {
    use std::error::Error;

    use super::PrintError;
    use ncl_object::ObjectError;

    #[test]
    fn display_and_source_cover_error_variants() {
        let object = PrintError::Object(ObjectError::Layout);
        assert!(object.to_string().contains("object error"));
        assert!(object.source().is_some());
        assert_eq!(
            PrintError::Sink("full".into()).to_string(),
            "print: sink error: full"
        );
        assert_eq!(
            PrintError::NotReadable.to_string(),
            "print: object is not readable"
        );
        assert!(PrintError::Circularity.to_string().contains("circular"));
        assert!(PrintError::Sink("full".into()).source().is_none());
    }
}

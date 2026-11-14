//! Typed failures from the Common Lisp stream context.

use ncl_object::typed::ConditionErrorData;
use ncl_object::{LispError, StreamError as LispStreamError, Word};

/// A stream operation failure with the affected stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum StreamError {
    /// The operation used a closed stream.
    Closed {
        /// The affected stream.
        stream: Word,
    },
    /// The stream does not support the requested direction.
    InvalidDirection {
        /// The affected stream.
        stream: Word,
    },
    /// An input or output operation failed.
    Io {
        /// The affected stream.
        stream: Word,
    },
    /// Input ended before the operation could complete.
    EndOfFile {
        /// The stream that reached end of file.
        stream: Word,
    },
}

impl std::fmt::Display for StreamError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed { .. } => formatter.write_str("stream is closed"),
            Self::InvalidDirection { .. } => formatter.write_str("invalid stream direction"),
            Self::Io { .. } => formatter.write_str("stream input/output failed"),
            Self::EndOfFile { .. } => formatter.write_str("end of file"),
        }
    }
}

impl std::error::Error for StreamError {}

impl From<StreamError> for LispError {
    fn from(error: StreamError) -> Self {
        match error {
            StreamError::Closed { stream } => Self::Condition(ConditionErrorData::StreamError {
                kind: LispStreamError::Closed,
                stream,
            }),
            StreamError::InvalidDirection { stream } => {
                Self::Condition(ConditionErrorData::StreamError {
                    kind: LispStreamError::InvalidDirection,
                    stream,
                })
            }
            StreamError::Io { stream } => Self::Condition(ConditionErrorData::StreamError {
                kind: LispStreamError::Io,
                stream,
            }),
            StreamError::EndOfFile { stream } => {
                Self::Condition(ConditionErrorData::EndOfFile { stream })
            }
        }
    }
}

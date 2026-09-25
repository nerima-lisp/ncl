//! Errors returned while saving or loading an NCL image.

use ncl_object::ObjectError;
use ncl_sys::CodeError;

/// Failure while encoding, decoding, saving, or loading an image.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ImageError {
    /// The host operating system could not read or write an image file.
    Io(std::io::ErrorKind),
    /// The byte stream ended before a complete value could be read.
    Truncated {
        /// Byte offset at which the read was attempted.
        offset: usize,
        /// Number of bytes the reader required.
        needed: usize,
    },
    /// The leading magic bytes did not match an NCL image.
    BadMagic,
    /// The image was written by an incompatible format version.
    UnsupportedVersion {
        /// Version found in the image header.
        found: u16,
        /// Version this build supports.
        supported: u16,
    },
    /// A fixed header field did not hold its contract value.
    InvalidField {
        /// Name of the offending field.
        field: &'static str,
    },
    /// A record or reference tag byte is not defined by this format.
    UnknownTag {
        /// Name of the tag space.
        space: &'static str,
        /// Tag byte that was read.
        tag: u8,
    },
    /// A heap object kind cannot be serialized.
    UnsupportedKind {
        /// Human-readable kind name.
        kind: &'static str,
    },
    /// A count or offset cannot be represented in the image layout.
    InvalidLayout {
        /// Name of the offending quantity.
        field: &'static str,
    },
    /// The object layer reported a failure.
    Object(ObjectError),
    /// The code space reported a failure.
    Code(CodeError),
}

impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "image file I/O failed: {error}"),
            Self::Truncated { offset, needed } => {
                write!(f, "image truncated at {offset}, needed {needed} bytes")
            }
            Self::BadMagic => f.write_str("not an NCL image"),
            Self::UnsupportedVersion { found, supported } => {
                write!(f, "image version {found}, this build supports {supported}")
            }
            Self::InvalidField { field } => write!(f, "invalid image field: {field}"),
            Self::UnknownTag { space, tag } => write!(f, "unknown {space} tag {tag}"),
            Self::UnsupportedKind { kind } => write!(f, "unsupported object kind: {kind}"),
            Self::InvalidLayout { field } => write!(f, "image layout overflow: {field}"),
            Self::Object(error) => write!(f, "object error: {error}"),
            Self::Code(error) => write!(f, "code error: {error:?}"),
        }
    }
}

impl std::error::Error for ImageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Object(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ImageError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.kind())
    }
}

impl From<ObjectError> for ImageError {
    fn from(error: ObjectError) -> Self {
        Self::Object(error)
    }
}

impl From<CodeError> for ImageError {
    fn from(error: CodeError) -> Self {
        Self::Code(error)
    }
}

use std::fmt;

/// Stable reason why a native FASL was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FaslRejectReason {
    /// The eight-byte FASL magic does not match.
    BadMagic,
    /// The container or semantic format version is unsupported.
    UnsupportedVersion,
    /// The file target differs from the expected target.
    TargetMismatch,
    /// The runtime ABI revision differs from the expected revision.
    RuntimeAbiMismatch,
    /// The compiler identity differs from the expected identity.
    CompilerIdentityMismatch,
    /// The policy fingerprint does not match its metadata or expected value.
    PolicyFingerprintMismatch,
    /// The feature fingerprint does not match its metadata or expected value.
    FeatureFingerprintMismatch,
    /// The section directory or one of its ranges is malformed.
    MalformedDirectory,
    /// A file, record, or string exceeds a format limit.
    ResourceLimitExceeded,
    /// The metadata section is malformed or non-canonical.
    MalformedMetadata,
    /// A record references an unknown or out-of-range ID.
    InvalidReference,
    /// A code object is malformed or inconsistent with its other records.
    InvalidCodeObject,
    /// A relocation is malformed or invalid for its target.
    InvalidRelocation,
    /// A stack map is malformed or inconsistent with its code object.
    InvalidSafepointMap,
    /// A debug record is malformed or inconsistent with its source or code.
    InvalidDebugRecord,
    /// A top-level record is malformed or out of source order.
    InvalidTopLevelRecord,
}

impl FaslRejectReason {
    /// Returns the stable Lisp keyword name associated with this reason.
    #[must_use]
    pub const fn keyword(self) -> &'static str {
        match self {
            Self::BadMagic => ":BAD-MAGIC",
            Self::UnsupportedVersion => ":UNSUPPORTED-VERSION",
            Self::TargetMismatch => ":TARGET-MISMATCH",
            Self::RuntimeAbiMismatch => ":RUNTIME-ABI-MISMATCH",
            Self::CompilerIdentityMismatch => ":COMPILER-MISMATCH",
            Self::PolicyFingerprintMismatch => ":POLICY-MISMATCH",
            Self::FeatureFingerprintMismatch => ":FEATURE-MISMATCH",
            Self::MalformedDirectory => ":INVALID-SECTION",
            Self::ResourceLimitExceeded => ":RESOURCE-LIMIT",
            Self::MalformedMetadata => ":INVALID-METADATA",
            Self::InvalidReference => ":INVALID-REFERENCE",
            Self::InvalidCodeObject => ":INVALID-CODE-OBJECT",
            Self::InvalidRelocation => ":INVALID-RELOCATION",
            Self::InvalidSafepointMap => ":INVALID-STACK-MAP",
            Self::InvalidDebugRecord => ":INVALID-DEBUG-RECORD",
            Self::InvalidTopLevelRecord => ":INVALID-TOPLEVEL-RECORD",
        }
    }
}

/// A typed rejection from the native FASL reader or writer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FaslError {
    /// Stable category for runtime condition mapping.
    pub reason: FaslRejectReason,
    /// Byte offset of the rejected input field when it can be identified.
    pub offset: Option<usize>,
}

impl FaslError {
    /// Creates a rejection without an input offset.
    #[must_use]
    pub const fn new(reason: FaslRejectReason) -> Self {
        Self {
            reason,
            offset: None,
        }
    }

    /// Creates a rejection at a byte offset in the serialized input.
    #[must_use]
    pub const fn at(reason: FaslRejectReason, offset: usize) -> Self {
        Self {
            reason,
            offset: Some(offset),
        }
    }
}

impl fmt::Display for FaslError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.reason.keyword())?;
        if let Some(offset) = self.offset {
            write!(f, " at byte {offset}")?;
        }
        Ok(())
    }
}

impl std::error::Error for FaslError {}

/// Errors produced while encoding or validating an object artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObjectError {
    /// The input ended before a required field was available.
    Truncated {
        /// Offset where bytes ended.
        offset: usize,
        /// Number of bytes required.
        needed: usize,
    },
    /// A field contained a value not supported by the format.
    InvalidField {
        /// Field name.
        field: &'static str,
        /// Rejected value.
        value: u64,
    },
    /// A section range was outside the input bytes.
    OutOfBounds {
        /// Section or table name.
        section: &'static str,
        /// Start offset.
        offset: u64,
        /// Requested byte count.
        size: u64,
    },
    /// Two sections overlap.
    Overlap {
        /// Earlier range.
        first: &'static str,
        /// Later overlapping range.
        second: &'static str,
    },
    /// A relocation refers to an unknown section or symbol.
    InvalidReference {
        /// Reference category.
        kind: &'static str,
        /// Invalid index.
        index: usize,
    },
    /// A relocation cannot be represented in the target format.
    UnsupportedRelocation(RelocKind),
    /// A name or table cannot be represented in the target format.
    InvalidName,
    /// The input has a valid container header but invalid internal structure.
    InvalidStructure(&'static str),
}

use crate::RelocKind;

impl fmt::Display for ObjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { offset, needed } => {
                write!(f, "truncated input at {offset}, need {needed} bytes")
            }
            Self::InvalidField { field, value } => write!(f, "invalid {field}: {value}"),
            Self::OutOfBounds {
                section,
                offset,
                size,
            } => write!(
                f,
                "{section} range {offset}..{} is out of bounds",
                offset.saturating_add(*size)
            ),
            Self::Overlap { first, second } => write!(f, "sections {first} and {second} overlap"),
            Self::InvalidReference { kind, index } => write!(f, "invalid {kind} reference {index}"),
            Self::UnsupportedRelocation(kind) => write!(f, "unsupported relocation {kind:?}"),
            Self::InvalidName => f.write_str("name cannot be represented"),
            Self::InvalidStructure(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ObjectError {}

#[cfg(test)]
mod private_tests {
    use super::{FaslError, FaslRejectReason};

    #[test]
    fn fasl_error_formats_keyword_with_optional_offset() {
        assert_eq!(
            FaslError::new(FaslRejectReason::BadMagic).to_string(),
            ":BAD-MAGIC"
        );
        assert_eq!(
            FaslError::at(FaslRejectReason::MalformedMetadata, 12).to_string(),
            ":INVALID-METADATA at byte 12"
        );
    }
}

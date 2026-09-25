//! Validated image-domain values.

#![allow(
    missing_docs,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::missing_const_for_fn,
    reason = "small validated value accessors"
)]

use crate::error::ImageError;

/// The on-disk image format version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Version(u16);

impl Version {
    /// The only version understood by this crate.
    pub const CURRENT: Self = Self(1);

    pub const fn get(self) -> u16 {
        self.0
    }

    pub fn parse(value: u16) -> Result<Self, ImageError> {
        if value == Self::CURRENT.0 {
            Ok(Self(value))
        } else {
            Err(ImageError::UnsupportedVersion {
                found: value,
                supported: Self::CURRENT.0,
            })
        }
    }
}

/// An image target architecture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Architecture {
    X86_64,
    Aarch64,
}

impl Architecture {
    pub const fn tag(self) -> u8 {
        match self {
            Self::X86_64 => 1,
            Self::Aarch64 => 2,
        }
    }

    pub fn parse(tag: u8) -> Result<Self, ImageError> {
        match tag {
            1 => Ok(Self::X86_64),
            2 => Ok(Self::Aarch64),
            tag => Err(ImageError::UnknownTag {
                space: "architecture",
                tag,
            }),
        }
    }

    pub const fn host() -> Self {
        if cfg!(target_arch = "x86_64") {
            Self::X86_64
        } else {
            Self::Aarch64
        }
    }
}

/// A checked 32-bit byte offset in the image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Offset(u32);

impl Offset {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// A checked 32-bit byte size in the image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Size(u32);

impl Size {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// A byte range occupied by one payload section.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Section {
    pub offset: Offset,
    pub size: Size,
}

impl Section {
    pub fn end(self) -> Result<usize, ImageError> {
        usize::try_from(self.offset.get())
            .ok()
            .and_then(|offset| offset.checked_add(self.size.get() as usize))
            .ok_or(ImageError::InvalidLayout {
                field: "payload bounds",
            })
    }
}

/// Feature names captured with an image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Features(Vec<String>);

impl Features {
    pub fn new(values: Vec<String>) -> Result<Self, ImageError> {
        if values.iter().any(|value| value.contains('\0')) {
            return Err(ImageError::InvalidField { field: "feature" });
        }
        Ok(Self(values))
    }

    pub fn as_slice(&self) -> &[String] {
        &self.0
    }
    pub fn into_inner(self) -> Vec<String> {
        self.0
    }
}

/// Fixed metadata needed to interpret the payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Header {
    pub version: Version,
    pub architecture: Architecture,
    pub payload: Section,
}

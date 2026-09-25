//! File and byte adapters for the image domain.

#![allow(clippy::missing_errors_doc, reason = "adapter errors are ImageError")]

use std::path::Path;

use crate::error::ImageError;
use crate::format::ImageFile;

/// Parse one complete image from bytes.
#[must_use]
pub fn parse(bytes: &[u8]) -> Result<ImageFile, ImageError> {
    ImageFile::from_bytes(bytes)
}

/// Read and parse an image file.
#[must_use]
pub fn read(path: impl AsRef<Path>) -> Result<ImageFile, ImageError> {
    let bytes = std::fs::read(path).map_err(|error| ImageError::Io(error.kind()))?;
    parse(&bytes)
}

/// Serialize and write an image file.
#[must_use]
pub fn write(path: impl AsRef<Path>, image: &ImageFile) -> Result<(), ImageError> {
    std::fs::write(path, image.to_bytes()?).map_err(|error| ImageError::Io(error.kind()))
}

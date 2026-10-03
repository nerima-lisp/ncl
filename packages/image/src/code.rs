//! Machine-code blobs captured from `ncl-sys` code space.

use ncl_sys::{CodeError, CodePtr, alloc_code, publish_code, write_code};

use crate::error::ImageError;

/// A published machine-code block saved in an image.
///
/// An image stores the raw bytes, the entry offset, the native frame size, and
/// the Lisp function name. Loading republishes the bytes into a fresh,
/// non-moving code allocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeImage {
    bytes: Vec<u8>,
    entry_offset: u32,
    frame_words: u16,
    function_name: String,
}

impl CodeImage {
    /// Capture the published bytes of a code block.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError::Code`] with `NotPublished` when the block is still
    /// writable, and [`ImageError::InvalidLayout`] when the entry offset does
    /// not fit the image field.
    pub fn capture(
        code: &CodePtr,
        entry_offset: usize,
        frame_words: u16,
        function_name: &str,
    ) -> Result<Self, ImageError> {
        if !code.is_published() {
            return Err(ImageError::Code(CodeError::NotPublished));
        }
        Self::from_raw(
            code.as_slice().to_vec(),
            entry_offset,
            frame_words,
            function_name.to_owned(),
        )
    }

    /// Build a code image from raw bytes.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError::InvalidLayout`] when the entry offset is not
    /// representable or lies beyond the byte buffer.
    pub fn from_raw(
        bytes: Vec<u8>,
        entry_offset: usize,
        frame_words: u16,
        function_name: String,
    ) -> Result<Self, ImageError> {
        if entry_offset > bytes.len() {
            return Err(ImageError::InvalidLayout {
                field: "code entry offset",
            });
        }
        let entry_offset = u32::try_from(entry_offset).map_err(|_| ImageError::InvalidLayout {
            field: "code entry offset",
        })?;
        Ok(Self {
            bytes,
            entry_offset,
            frame_words,
            function_name,
        })
    }

    /// Return the raw code bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Return the entry byte offset within the allocation.
    #[must_use]
    pub const fn entry_offset(&self) -> usize {
        self.entry_offset as usize
    }

    /// Return the complete native frame size in words.
    #[must_use]
    pub const fn frame_words(&self) -> u16 {
        self.frame_words
    }

    /// Return the Lisp function name.
    #[must_use]
    pub fn function_name(&self) -> &str {
        &self.function_name
    }

    /// Copy these bytes into a fresh code allocation and publish them.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError::Code`] when allocation, writing, or publication
    /// fails.
    pub fn publish(&self) -> Result<CodePtr, ImageError> {
        let mut code = alloc_code(self.bytes.len())?;
        write_code(&mut code, 0, &self.bytes)?;
        publish_code(&mut code)?;
        Ok(code)
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::needless_pass_by_value,
    clippy::unwrap_used,
    reason = "tests assert on code-space failures"
)]
mod tests {
    use super::CodeImage;
    use crate::error::ImageError;

    #[test]
    fn raw_code_image_preserves_metadata_and_rejects_bad_offsets() {
        let image = CodeImage::from_raw(vec![1, 2, 3], 2, 7, "demo".to_owned()).unwrap();
        assert_eq!(image.bytes(), &[1, 2, 3]);
        assert_eq!(image.entry_offset(), 2);
        assert_eq!(image.frame_words(), 7);
        assert_eq!(image.function_name(), "demo");
        assert_eq!(
            CodeImage::from_raw(vec![1], 2, 0, String::new()),
            Err(ImageError::InvalidLayout {
                field: "code entry offset"
            })
        );
    }

    #[test]
    fn writable_code_capture_reports_not_published() {
        let code = ncl_sys::alloc_code(1).unwrap();
        assert_eq!(
            CodeImage::capture(&code, 0, 0, "unpublished"),
            Err(ImageError::Code(ncl_sys::CodeError::NotPublished))
        );
    }
}

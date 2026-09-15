//! Native frame layout contracts.

/// Number of words in the frame header.
pub const FRAME_HEADER_WORDS: u32 = 4;

/// Size of one machine word in bytes.
pub const WORD_BYTES: u32 = 8;

/// Stack layout for one generated function.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameLayout {
    /// Number of argument spill slots.
    pub argument_words: u32,
    /// Number of local value slots.
    pub local_words: u32,
    /// Number of outgoing call slots owned by this frame.
    pub outgoing_words: u32,
    /// Total frame size in words, including alignment padding.
    pub frame_words: u32,
}

impl FrameLayout {
    /// Builds a layout with a 16-byte-aligned total size.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CodegenError::FrameOverflow`] if the word count cannot
    /// be represented by `u32`.
    pub fn new(
        argument_words: u32,
        local_words: u32,
        outgoing_words: u32,
    ) -> Result<Self, crate::CodegenError> {
        let unaligned = FRAME_HEADER_WORDS
            .checked_add(argument_words)
            .and_then(|words| words.checked_add(local_words))
            .and_then(|words| words.checked_add(outgoing_words))
            .ok_or(crate::CodegenError::FrameOverflow)?;
        let frame_words = unaligned
            .checked_add(1)
            .ok_or(crate::CodegenError::FrameOverflow)?
            & !1;
        Ok(Self {
            argument_words,
            local_words,
            outgoing_words,
            frame_words,
        })
    }

    /// Returns the byte offset of the first local slot.
    #[must_use]
    pub const fn local_offset_bytes(self) -> u32 {
        (FRAME_HEADER_WORDS + self.argument_words) * WORD_BYTES
    }

    /// Returns the byte size of this frame.
    #[must_use]
    pub const fn size_bytes(self) -> u32 {
        self.frame_words * WORD_BYTES
    }
}

use crate::{FaslError, FaslRejectReason};

/// Code-object facts shared by section validators.
#[derive(Clone, Copy, Debug)]
pub struct FaslCodeLayout<'a> {
    pub id: u32,
    pub entry_offset: u32,
    pub frame_words: u16,
    pub constant_count: u32,
    pub code: &'a [u8],
}

/// Checked cursor for one bounded FASL section.
pub struct SectionReader<'a> {
    bytes: &'a [u8],
    base_offset: usize,
    cursor: usize,
    reason: FaslRejectReason,
}

impl<'a> SectionReader<'a> {
    pub const fn new(bytes: &'a [u8], base_offset: usize, reason: FaslRejectReason) -> Self {
        Self {
            bytes,
            base_offset,
            cursor: 0,
            reason,
        }
    }

    pub const fn absolute_position(&self) -> usize {
        self.base_offset.saturating_add(self.cursor)
    }

    pub const fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.cursor)
    }

    pub fn count(
        &mut self,
        maximum: usize,
        minimum_record_size: usize,
    ) -> Result<usize, FaslError> {
        let offset = self.absolute_position();
        let count = usize::try_from(self.u32()?)
            .map_err(|_| FaslError::at(FaslRejectReason::ResourceLimitExceeded, offset))?;
        if count > maximum {
            return Err(FaslError::at(
                FaslRejectReason::ResourceLimitExceeded,
                offset,
            ));
        }
        if minimum_record_size != 0 && count > self.remaining() / minimum_record_size {
            return Err(FaslError::at(self.reason, offset));
        }
        Ok(count)
    }

    pub fn take(&mut self, length: usize) -> Result<&'a [u8], FaslError> {
        let start = self.cursor;
        let end = start
            .checked_add(length)
            .ok_or_else(|| FaslError::at(self.reason, self.base_offset.saturating_add(start)))?;
        let value = self
            .bytes
            .get(start..end)
            .ok_or_else(|| FaslError::at(self.reason, self.base_offset.saturating_add(start)))?;
        self.cursor = end;
        Ok(value)
    }

    pub fn u8(&mut self) -> Result<u8, FaslError> {
        self.take(1)?
            .first()
            .copied()
            .ok_or_else(|| FaslError::at(self.reason, self.absolute_position()))
    }

    pub fn u16(&mut self) -> Result<u16, FaslError> {
        Ok(u16::from_le_bytes(self.take_array()?))
    }

    pub fn u32(&mut self) -> Result<u32, FaslError> {
        Ok(u32::from_le_bytes(self.take_array()?))
    }

    pub fn u64(&mut self) -> Result<u64, FaslError> {
        Ok(u64::from_le_bytes(self.take_array()?))
    }

    pub fn i64(&mut self) -> Result<i64, FaslError> {
        Ok(i64::from_le_bytes(self.take_array()?))
    }

    fn take_array<const N: usize>(&mut self) -> Result<[u8; N], FaslError> {
        let source = self.take(N)?;
        let mut bytes = [0; N];
        bytes.copy_from_slice(source);
        Ok(bytes)
    }

    pub fn zero_padding(&mut self, alignment: usize) -> Result<(), FaslError> {
        let padding = alignment.wrapping_sub(self.cursor % alignment) % alignment;
        let offset = self.absolute_position();
        if let Some(index) = self.take(padding)?.iter().position(|byte| *byte != 0) {
            return Err(FaslError::at(self.reason, offset.saturating_add(index)));
        }
        Ok(())
    }

    pub const fn finish(self) -> Result<(), FaslError> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(FaslError::at(self.reason, self.absolute_position()))
        }
    }
}

/// Deterministic little-endian writer for one FASL section.
#[derive(Default)]
pub struct SectionWriter {
    bytes: Vec<u8>,
}

impl SectionWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn put_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub fn put_u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn put_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn put_u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn put_i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn put_bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    pub fn zero_padding(&mut self, alignment: usize) {
        let padding = alignment.wrapping_sub(self.bytes.len() % alignment) % alignment;
        self.bytes.resize(self.bytes.len() + padding, 0);
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

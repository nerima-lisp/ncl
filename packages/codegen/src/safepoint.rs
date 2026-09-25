#![allow(missing_docs)]

use core::fmt;

pub const HEADER_SIZE: usize = 16;
pub const HEADER_WORD_COUNT: u16 = 4;
pub const FLAG_CALL: u32 = 1 << 0;
pub const FLAG_LOOP_BACKEDGE: u32 = 1 << 1;
pub const FLAG_ALLOCATION_SLOW: u32 = 1 << 2;
pub const FLAG_HAS_DERIVED_ADDRESS: u32 = 1 << 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafepointMap {
    pub pc_offset: u32,
    pub frame_words: u16,
    pub slot_words: u16,
    pub word_slot_count: u16,
    pub register_mask: u16,
    pub map_flags: u32,
    pub bitmap: Vec<u8>,
    pub registers: Vec<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MapError {
    FrameTooSmall,
    SlotCountOutOfRange,
    BitmapTooLarge,
    RegisterCountOutOfRange,
    InvalidBitmap,
    InvalidHeader,
    Truncated,
}

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::FrameTooSmall => "frame must contain the four-word header",
            Self::SlotCountOutOfRange => "word slot count exceeds bitmap capacity",
            Self::BitmapTooLarge => "bitmap capacity does not fit in the wire header",
            Self::RegisterCountOutOfRange => "register count exceeds the register mask",
            Self::InvalidBitmap => "bitmap does not contain the required header bits",
            Self::InvalidHeader => "safepoint header is inconsistent",
            Self::Truncated => "truncated safepoint map",
        };
        f.write_str(message)
    }
}

impl std::error::Error for MapError {}

impl SafepointMap {
    /// Builds the wire-format mask for the live register ids.
    ///
    /// Register ids are shared by the x86-64 and `AArch64` backends and are
    /// therefore represented as `u16`. Only ids addressable by the 16-bit
    /// mask are accepted, and an id may occur only once.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicate or unrepresentable ids.
    pub fn build_register_mask(registers: &[u16]) -> Result<u16, MapError> {
        let mut register_mask = 0u16;
        for &register in registers {
            if u32::from(register) >= u16::BITS {
                return Err(MapError::RegisterCountOutOfRange);
            }
            let bit = 1u16 << register;
            if register_mask & bit != 0 {
                return Err(MapError::InvalidHeader);
            }
            register_mask |= bit;
        }
        Ok(register_mask)
    }

    /// Verifies that a mask describes exactly the supplied live register ids.
    ///
    /// # Errors
    ///
    /// Returns an error when the ids do not equal the mask.
    pub fn validate_register_mask(register_mask: u16, registers: &[u16]) -> Result<(), MapError> {
        if Self::build_register_mask(registers)? != register_mask {
            return Err(MapError::InvalidHeader);
        }
        Ok(())
    }

    /// Constructs a safepoint map using the fixed 16-byte header format.
    ///
    /// # Errors
    ///
    /// Returns an error when frame, slot, bitmap, or register limits are
    /// exceeded or the register list contains a duplicate.
    pub fn new(
        pc_offset: u32,
        frame_words: u16,
        slot_words: u16,
        live_slots: &[u16],
        registers: &[u16],
        map_flags: u32,
    ) -> Result<Self, MapError> {
        if frame_words < HEADER_WORD_COUNT {
            return Err(MapError::FrameTooSmall);
        }
        let bitmap_len = usize::from(slot_words).div_ceil(8);
        let mut bitmap = vec![0; bitmap_len];
        set_bit(&mut bitmap, 2)?;
        for &slot in live_slots {
            if slot >= slot_words {
                return Err(MapError::SlotCountOutOfRange);
            }
            set_bit(&mut bitmap, slot)?;
        }
        if live_slots.len() > usize::from(u16::MAX) {
            return Err(MapError::SlotCountOutOfRange);
        }
        let register_mask = Self::build_register_mask(registers)?;
        Ok(Self {
            pc_offset,
            frame_words,
            slot_words,
            word_slot_count: slot_words,
            register_mask,
            map_flags,
            bitmap,
            registers: registers.to_vec(),
        })
    }

    /// Serializes the map into its wire representation.
    ///
    /// # Errors
    ///
    /// Returns an error when the map violates the fixed wire format.
    pub fn encode(&self) -> Result<Vec<u8>, MapError> {
        self.validate()?;
        let mut bytes =
            Vec::with_capacity(HEADER_SIZE + self.bitmap.len() + self.registers.len() * 2);
        bytes.extend_from_slice(&self.pc_offset.to_le_bytes());
        bytes.extend_from_slice(&self.frame_words.to_le_bytes());
        bytes.extend_from_slice(&self.slot_words.to_le_bytes());
        bytes.extend_from_slice(&self.word_slot_count.to_le_bytes());
        bytes.extend_from_slice(&self.register_mask.to_le_bytes());
        bytes.extend_from_slice(&self.map_flags.to_le_bytes());
        bytes.extend_from_slice(&self.bitmap);
        for register in &self.registers {
            bytes.extend_from_slice(&register.to_le_bytes());
        }
        Ok(bytes)
    }

    /// Validates the in-memory map against the wire-format invariants.
    ///
    /// # Errors
    ///
    /// Returns an error when the bitmap, header, or register mask is invalid.
    pub fn validate(&self) -> Result<(), MapError> {
        if self.frame_words < HEADER_WORD_COUNT {
            return Err(MapError::FrameTooSmall);
        }
        if self.bitmap.len() != usize::from(self.slot_words).div_ceil(8)
            || self.word_slot_count > self.slot_words
            || !bit_is_set(&self.bitmap, 2)
            || bit_is_set(&self.bitmap, 0)
            || bit_is_set(&self.bitmap, 1)
            || bit_is_set(&self.bitmap, 3)
        {
            return Err(MapError::InvalidBitmap);
        }
        Self::validate_register_mask(self.register_mask, &self.registers)?;
        Ok(())
    }
}

fn set_bit(bitmap: &mut [u8], bit: u16) -> Result<(), MapError> {
    let index = usize::from(bit) / 8;
    let mask = 1u8 << (bit % 8);
    let Some(byte) = bitmap.get_mut(index) else {
        return Err(MapError::SlotCountOutOfRange);
    };
    *byte |= mask;
    Ok(())
}

fn bit_is_set(bitmap: &[u8], bit: u16) -> bool {
    bitmap
        .get(usize::from(bit) / 8)
        .is_some_and(|byte| byte & (1u8 << (bit % 8)) != 0)
}

#[cfg(test)]
mod tests {
    use super::{MapError, SafepointMap};

    #[test]
    fn builds_masks_for_common_u16_register_ids() {
        let mask = SafepointMap::build_register_mask(&[0, 3, 14, 15]);
        assert_eq!(mask, Ok(0xC009));
    }

    #[test]
    fn rejects_duplicate_and_unrepresentable_register_ids() {
        assert_eq!(
            SafepointMap::build_register_mask(&[3, 3]),
            Err(MapError::InvalidHeader)
        );
        assert_eq!(
            SafepointMap::build_register_mask(&[16]),
            Err(MapError::RegisterCountOutOfRange)
        );
    }

    #[test]
    fn validates_register_ids_against_the_wire_mask() {
        assert!(SafepointMap::validate_register_mask(0x0009, &[0, 3]).is_ok());
        assert_eq!(
            SafepointMap::validate_register_mask(0x0009, &[0, 4]),
            Err(MapError::InvalidHeader)
        );

        let Ok(mut map) = SafepointMap::new(0, 4, 4, &[], &[0, 3], 0) else {
            return;
        };
        map.registers = vec![0, 4];
        assert_eq!(map.validate(), Err(MapError::InvalidHeader));
        assert_eq!(map.encode(), Err(MapError::InvalidHeader));
    }
}

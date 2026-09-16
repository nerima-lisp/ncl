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

    fn validate(&self) -> Result<(), MapError> {
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
        if self.registers.len() > usize::from(u16::MAX) + 1
            || self.registers.len() != self.register_mask.count_ones() as usize
        {
            return Err(MapError::InvalidHeader);
        }
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

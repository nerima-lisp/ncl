//! Object widetag and payload layout constants.

/// Convert payload-relative reference slots to raw heap indices.
#[must_use]
pub fn reference_words(payload_slots: &[usize]) -> Vec<usize> {
    payload_slots.iter().map(|slot| slot + 1).collect()
}

/// Object widetags used by the object layer.
pub mod widetag {
    pub const SYMBOL: u8 = 1;
    pub const STRING: u8 = 2;
    pub const SIMPLE_VECTOR: u8 = 3;
    pub const HASH_TABLE: u8 = 5;
    pub const STRUCTURE: u8 = 6;
    pub const INSTANCE: u8 = 7;
    pub const SIMPLE_FUN: u8 = 8;
    pub const CLOSURE: u8 = 9;
    pub const BIGNUM: u8 = 10;
    pub const RATIO: u8 = 11;
    pub const DOUBLE_FLOAT: u8 = 12;
    pub const COMPLEX: u8 = 13;
    pub const PACKAGE: u8 = 14;
    pub const READTABLE: u8 = 15;
    pub const STREAM: u8 = 16;
    pub const CODE: u8 = 17;
    pub const SPECIALIZED_ARRAY: u8 = 18;
    pub const NON_SIMPLE_ARRAY: u8 = 19;
}

pub mod string_offset {
    pub const LENGTH: usize = 0;
    pub const DATA: usize = 1;
}

pub mod simple_vector_offset {
    pub const LENGTH: usize = 0;
    pub const DATA: usize = 1;
}

pub mod specialized_array_offset {
    pub const ELEMENT_TYPE: usize = 0;
    pub const LENGTH: usize = 1;
    pub const DATA: usize = 2;
}

pub mod array_offset {
    pub const ELEMENT_TYPE: usize = 0;
    pub const RANK: usize = 1;
    pub const DIMENSIONS: usize = 2;
    pub const DYNAMIC_BASE: usize = 2;
    pub const FLAG_ADJUSTABLE: u64 = 1;
    pub const FLAG_HAS_FILL_POINTER: u64 = 2;
    pub const FLAG_DISPLACED: u64 = 4;
}

/// Payload offsets for a symbol object.
pub mod symbol_offset {
    pub const VALUE: usize = 0;
    pub const FUNCTION: usize = 1;
    pub const PLIST: usize = 2;
    pub const PACKAGE: usize = 3;
    pub const NAME: usize = 4;
    pub const TLS_INDEX: usize = 5;
    pub const HASH: usize = 6;
    pub const FLAGS: usize = 7;
}

/// Flag bit positions in a symbol's flags word.
pub mod symbol_flag {
    pub const SPECIAL: u32 = 1;
    pub const CONSTANT: u32 = 2;
    pub const MACRO: u32 = 4;
    pub const PACKAGE_LOCKED: u32 = 8;
}

pub mod structure_offset {
    pub const LAYOUT: usize = 0;
    pub const SLOTS: usize = 1;
}

pub mod instance_offset {
    pub const CLASS: usize = 0;
    pub const SLOT_VECTOR: usize = 1;
    pub const GENERATION: usize = 2;
}

pub mod function_offset {
    pub const ENTRY: usize = 0;
    pub const NAME: usize = 1;
    pub const LAMBDA_LIST: usize = 2;
    pub const CODE: usize = 3;
    pub const CAPTURES: usize = 4;
}

pub mod number_offset {
    pub const SIGN: usize = 0;
    pub const LIMB_COUNT: usize = 1;
    pub const LIMBS: usize = 2;
    pub const RATIO_NUMERATOR: usize = 0;
    pub const RATIO_DENOMINATOR: usize = 1;
    pub const DOUBLE_BITS: usize = 0;
    pub const COMPLEX_REAL: usize = 0;
    pub const COMPLEX_IMAG: usize = 1;
}

pub mod stream_offset {
    pub const DIRECTION: usize = 0;
    pub const ELEMENT_TYPE: usize = 1;
    pub const EXTERNAL_FORMAT: usize = 2;
    pub const STATE: usize = 3;
    pub const IMPLEMENTATION: usize = 4;
}

pub mod readtable_offset {
    pub const SYNTAX: usize = 0;
    pub const DISPATCH: usize = 1;
    pub const CASE: usize = 2;
}

pub mod code_offset {
    pub const ENTRY: usize = 0;
    pub const SIZE: usize = 1;
    pub const CONSTANTS: usize = 2;
    pub const STACK_MAP: usize = 3;
    pub const DEBUG: usize = 4;
}

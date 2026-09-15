//! Object widetag and payload layout constants.

/// Object widetags used by the object layer.
pub mod widetag {
    pub const SYMBOL: u8 = 1;
    pub const STRING: u8 = 2;
    pub const SIMPLE_VECTOR: u8 = 3;
    pub const ARRAY: u8 = 4;
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
    pub const FILL_POINTER: usize = 3;
    pub const DISPLACED_TO: usize = 4;
    pub const OFFSET: usize = 5;
    pub const FLAGS: usize = 6;
    pub const DATA: usize = 7;
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

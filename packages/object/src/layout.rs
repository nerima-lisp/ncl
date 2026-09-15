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

use core::fmt;

/// The lowtag carried by a tagged NCL value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum LowTag {
    Character = 0,
    List = 1,
    SingleFloat = 2,
    Function = 3,
    OtherImmediate = 4,
    Instance = 5,
    Reserved = 6,
    OtherPointer = 7,
}

/// A 64-bit NCL tagged value.
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct Word(u64);

impl fmt::Debug for Word {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Word({:#x})", self.0)
    }
}

impl Word {
    /// The canonical NIL value.
    pub const NIL: Self = Self(1);
    /// The canonical true value, represented as an other pointer placeholder until object layout is registered.
    pub const TRUE: Self = Self(7);
    /// The unbound marker.
    pub const UNBOUND: Self = Self(4);
    /// Encode a signed 63-bit fixnum.
    pub const fn fixnum(value: i64) -> Self {
        Self((value as u64) << 1)
    }
    /// Decode a fixnum when the low bit is zero.
    pub const fn as_fixnum(self) -> Option<i64> {
        if self.0 & 1 == 0 {
            Some((self.0 as i64) >> 1)
        } else {
            None
        }
    }
    /// Encode a character in bits 4..24.
    pub const fn character(value: u32) -> Self {
        Self(((value as u64) << 4) | 1)
    }
    /// Encode a pointer with a lowtag.
    pub const fn pointer(address: usize, tag: LowTag) -> Self {
        Self((address as u64) | tag as u64)
    }
    /// Return the raw tagged bits.
    pub const fn bits(self) -> u64 {
        self.0
    }
    /// Construct a tagged word from its serialized bits.
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }
    /// Return the lowtag.
    pub const fn lowtag(self) -> u8 {
        (self.0 & 7) as u8
    }
    /// Return the untagged address.
    pub const fn address(self) -> usize {
        (self.0 & !7) as usize
    }
    /// Whether this is a list value, including NIL.
    pub const fn is_list(self) -> bool {
        self.lowtag() == LowTag::List as u8
    }
    /// Whether this is a non-NIL cons pointer.
    pub const fn is_cons(self) -> bool {
        self.is_list() && self.0 != Self::NIL.0
    }
    /// Whether this is a fixnum.
    pub const fn is_fixnum(self) -> bool {
        self.0 & 1 == 0
    }
    /// Whether this is an immediate character.
    pub const fn is_character(self) -> bool {
        self.lowtag() == LowTag::Character as u8
    }
}

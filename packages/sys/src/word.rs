use core::fmt;

/// The lowtag carried by a tagged NCL value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum LowTag {
    /// Immediate character or character-like value.
    Character = 0,
    /// Cons pointer, including the NIL singleton.
    List = 1,
    /// Immediate single-float representation.
    SingleFloat = 2,
    /// Function object pointer.
    Function = 3,
    /// Other immediate value.
    OtherImmediate = 4,
    /// Instance pointer.
    Instance = 5,
    /// Reserved tag value.
    Reserved = 6,
    /// General heap pointer.
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
    #[must_use]
    pub const fn fixnum(value: i64) -> Self {
        Self(u64::from_ne_bytes(value.to_ne_bytes()) << 1)
    }
    /// Decode a fixnum when the low bit is zero.
    #[must_use]
    pub const fn as_fixnum(self) -> Option<i64> {
        if self.0 & 1 == 0 {
            Some(i64::from_ne_bytes(self.0.to_ne_bytes()) >> 1)
        } else {
            None
        }
    }
    /// Encode a character in bits 4..24.
    #[must_use]
    pub const fn character(value: u32) -> Self {
        Self((value as u64) << 4)
    }
    /// Encode a pointer with a lowtag.
    #[must_use]
    pub const fn pointer(address: usize, tag: LowTag) -> Self {
        Self((address as u64) | tag as u64)
    }
    /// Return the raw tagged bits.
    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }
    /// Construct a tagged word from its serialized bits.
    #[must_use]
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }
    /// Return the lowtag.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "lowtag is defined as the low three bits"
    )]
    pub const fn lowtag(self) -> u8 {
        (self.0 & 7) as u8
    }
    /// Return the untagged address.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "heap addresses are usize-sized on supported targets"
    )]
    pub const fn address(self) -> usize {
        (self.0 & !7) as usize
    }
    /// Whether this is a list value, including NIL.
    #[must_use]
    pub const fn is_list(self) -> bool {
        self.lowtag() == LowTag::List as u8
    }
    /// Whether this is a non-NIL cons pointer.
    #[must_use]
    pub const fn is_cons(self) -> bool {
        self.is_list() && self.0 != Self::NIL.0
    }
    /// Whether this is a fixnum.
    #[must_use]
    pub const fn is_fixnum(self) -> bool {
        self.0 & 1 == 0
    }
    /// Whether this is an immediate character.
    #[must_use]
    pub const fn is_character(self) -> bool {
        self.lowtag() == LowTag::Character as u8
            && self.bits() != Self::NIL.bits()
            && self.address() < (1_usize << 32)
    }
}

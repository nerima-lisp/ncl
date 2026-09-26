use core::fmt;

/// Number of lowtag bits in a tagged word.
pub const LOWTAG_BITS: u32 = 3;
/// Mask selecting the lowtag bits.
pub const LOWTAG_MASK: u64 = (1_u64 << LOWTAG_BITS) - 1;
/// Number of payload bits reserved for a fixnum tag.
pub const FIXNUM_TAG_BITS: u32 = 1;
/// Bit pattern carried by a fixnum tag.
pub const FIXNUM_TAG: u64 = 0;
/// Number of payload bits before a character code.
pub const CHARACTER_SHIFT: u32 = 4;
/// Lowtag carried by an encoded character.
pub const CHARACTER_TAG: u64 = 1;
/// Largest Unicode scalar value accepted by the character representation.
pub const CHARACTER_MAX: u32 = 0x10_FFFF;

/// The lowtag carried by a tagged NCL value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum LowTag {
    /// Cons pointer, including the NIL singleton.
    List = 1,
    /// Function object pointer.
    Function = 3,
    /// Instance pointer.
    Instance = 5,
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
    /// The reserved unbound immediate. It is outside the 32-bit character payload.
    pub const UNBOUND: Self = Self(0xffff_ffff_ffff_fff9);
    /// Encode a signed 63-bit fixnum.
    #[must_use]
    pub const fn fixnum(value: i64) -> Self {
        Self(u64::from_ne_bytes(value.to_ne_bytes()) << FIXNUM_TAG_BITS)
    }
    /// Decode a fixnum when the low bit is zero.
    #[must_use]
    pub const fn as_fixnum(self) -> Option<i64> {
        if self.0 & ((1_u64 << FIXNUM_TAG_BITS) - 1) == FIXNUM_TAG {
            Some(i64::from_ne_bytes(self.0.to_ne_bytes()) >> FIXNUM_TAG_BITS)
        } else {
            None
        }
    }
    /// Encode a character as `((code + 1) << 4) | 1`.
    #[must_use]
    pub const fn character(value: u32) -> Self {
        Self((((value as u64) + 1) << CHARACTER_SHIFT) | CHARACTER_TAG)
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
        (self.0 & LOWTAG_MASK) as u8
    }
    /// Return the untagged address.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "heap addresses are usize-sized on supported targets"
    )]
    pub const fn address(self) -> usize {
        (self.0 & !LOWTAG_MASK) as usize
    }
    /// Whether this is a list value, including NIL.
    #[must_use]
    pub const fn is_list(self) -> bool {
        self.lowtag() == LowTag::List as u8 && !self.is_character() && !self.is_unbound()
    }
    /// Whether this is a non-NIL cons pointer.
    #[must_use]
    pub const fn is_cons(self) -> bool {
        self.is_list() && self.0 != Self::NIL.0
    }
    /// Whether this is a fixnum.
    #[must_use]
    pub const fn is_fixnum(self) -> bool {
        self.0 & ((1_u64 << FIXNUM_TAG_BITS) - 1) == FIXNUM_TAG
    }
    /// Whether this is an immediate character.
    #[must_use]
    pub const fn is_character(self) -> bool {
        self.as_character().is_some()
    }
    /// Decode a Unicode scalar value from an immediate character.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "the preceding scalar-range check bounds the value to u32"
    )]
    pub const fn as_character(self) -> Option<u32> {
        if self.lowtag() != LowTag::List as u8
            || self.bits() == Self::NIL.bits()
            || self.bits() & ((1_u64 << CHARACTER_SHIFT) - 1) != CHARACTER_TAG
            || self.address() >= (1_usize << 32)
        {
            return None;
        }
        let encoded = self.address() as u64 >> CHARACTER_SHIFT;
        let Some(code) = encoded.checked_sub(1) else {
            return None;
        };
        if code <= CHARACTER_MAX as u64 {
            Some(code as u32)
        } else {
            None
        }
    }
    /// Whether this is the reserved unbound immediate.
    #[must_use]
    pub const fn is_unbound(self) -> bool {
        self.bits() == Self::UNBOUND.bits()
    }
}

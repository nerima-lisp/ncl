/// A checked fixnum operation attached to a generic builtin call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixnumOp {
    /// Add two fixnum operands.
    Add,
    /// Subtract the right fixnum operand from the left one.
    Sub,
    /// Multiply two fixnum operands.
    Mul,
    /// Compare two fixnum operands for a strict less-than relation.
    LessThan,
    /// Increment one fixnum operand.
    OnePlus,
    /// Decrement one fixnum operand.
    OneMinus,
}

impl FixnumOp {
    /// The fixed operand count accepted by direct builtin lowering.
    #[must_use]
    pub const fn arity(self) -> usize {
        match self {
            Self::OnePlus | Self::OneMinus => 1,
            Self::Add | Self::Sub | Self::Mul | Self::LessThan => 2,
        }
    }

    /// Resolve a directly lowered builtin name and arity.
    #[must_use]
    pub fn from_builtin(name: &str, arity: usize) -> Option<Self> {
        let operation = match (name, arity) {
            ("+", 2) => Self::Add,
            ("-", 2) => Self::Sub,
            ("*", 2) => Self::Mul,
            ("<", 2) => Self::LessThan,
            ("1+", 1) => Self::OnePlus,
            ("1-", 1) => Self::OneMinus,
            _ => return None, // check-added-lines: allow(wildcard) only listed names and arities have direct fixnum lowering.
        };
        Some(operation)
    }

    pub(crate) const fn from_tag(tag: u64) -> Option<Self> {
        match tag {
            0 => Some(Self::Add),
            1 => Some(Self::Sub),
            2 => Some(Self::Mul),
            3 => Some(Self::LessThan),
            4 => Some(Self::OnePlus),
            5 => Some(Self::OneMinus),
            _ => None, // check-added-lines: allow(wildcard) the parser rejects unknown serialized operation tags.
        }
    }

    pub(crate) const fn tag(self) -> u64 {
        match self {
            Self::Add => 0,
            Self::Sub => 1,
            Self::Mul => 2,
            Self::LessThan => 3,
            Self::OnePlus => 4,
            Self::OneMinus => 5,
        }
    }
}

/// Guard requirements for a checked specialization of a generic builtin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixnumSpecialization {
    operation: FixnumOp,
    guarded_operands: u8,
}

impl FixnumSpecialization {
    /// Create metadata from one guard bit per operation operand.
    #[must_use]
    pub fn new(operation: FixnumOp, guarded_operands: &[bool]) -> Option<Self> {
        if guarded_operands.len() != operation.arity() {
            return None;
        }
        let mut mask = 0_u8;
        for (index, guarded) in guarded_operands.iter().copied().enumerate() {
            if guarded {
                mask |= 1 << index;
            }
        }
        Some(Self {
            operation,
            guarded_operands: mask,
        })
    }

    pub(crate) const fn from_mask(operation: FixnumOp, mask: u8) -> Option<Self> {
        if mask & !((1 << operation.arity()) - 1) != 0 {
            return None;
        }
        Some(Self {
            operation,
            guarded_operands: mask,
        })
    }

    /// The operation code generation emits after its guards pass.
    #[must_use]
    pub const fn operation(self) -> FixnumOp {
        self.operation
    }

    /// Whether code generation must check the operand's fixnum tag.
    #[must_use]
    pub const fn needs_tag_check(self, index: usize) -> bool {
        index < self.operation.arity() && self.guarded_operands & (1 << index) != 0
    }

    pub(crate) const fn guard_mask(self) -> u8 {
        self.guarded_operands
    }
}

/// A conservative scalar category asserted by source type information.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarTypeHint {
    /// A signed integer that fits in the tagged fixnum payload.
    Fixnum,
    /// An integer that may require a bignum representation.
    Integer,
    /// A numeric value that may be non-integer.
    Number,
    /// A boolean result.
    Boolean,
    /// A value known not to be numeric.
    NonNumber,
}

use crate::EncodeError;
use crate::Label;

/// An architectural general-purpose register, x0 through x30.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Reg(pub u8);
impl From<Reg> for RegOrSp {
    fn from(value: Reg) -> Self {
        Self::Reg(value)
    }
}
impl Reg {
    /// Creates a register.
    pub const fn new(n: u8) -> Result<Self, EncodeError> {
        if n <= 30 {
            Ok(Self(n))
        } else {
            Err(EncodeError::InvalidRegister(n))
        }
    }
    /// Returns the register number.
    pub const fn number(self) -> u8 {
        self.0
    }
}
/// A register operand which may also be stack pointer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum RegOrSp {
    /// General register.
    Reg(Reg),
    /// Stack pointer.
    Sp,
}
/// A register operand which may also be zero register.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum RegOrZr {
    /// General register.
    Reg(Reg),
    /// Zero register.
    Zr,
}
/// A SIMD/floating-point register.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct VReg {
    /// Register number.
    pub number: u8,
    /// Register width, true for double precision.
    pub double: bool,
}
/// A condition code.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Cond {
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Carry set.
    Cs,
    /// Carry clear.
    Cc,
    /// Minus.
    Mi,
    /// Plus.
    Pl,
    /// Overflow.
    Vs,
    /// No overflow.
    Vc,
    /// Unsigned higher.
    Hi,
    /// Unsigned lower or same.
    Ls,
    /// Signed greater or equal.
    Ge,
    /// Signed less than.
    Lt,
    /// Signed greater than.
    Gt,
    /// Signed less or equal.
    Le,
    /// Always.
    Al,
}
impl Cond {
    pub(crate) const fn bits(self) -> u32 {
        match self {
            Self::Eq => 0,
            Self::Ne => 1,
            Self::Cs => 2,
            Self::Cc => 3,
            Self::Mi => 4,
            Self::Pl => 5,
            Self::Vs => 6,
            Self::Vc => 7,
            Self::Hi => 8,
            Self::Ls => 9,
            Self::Ge => 10,
            Self::Lt => 11,
            Self::Gt => 12,
            Self::Le => 13,
            Self::Al => 14,
        }
    }
}
/// A shifted register operand.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Shift {
    /// Logical shift left.
    Lsl(u8),
    /// Logical shift right.
    Lsr(u8),
    /// Arithmetic shift right.
    Asr(u8),
}
/// An extended register operand.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Extend {
    /// Unsigned byte.
    Uxtb,
    /// Unsigned halfword.
    Uxth,
    /// Unsigned word.
    Uxtw,
    /// Unsigned doubleword.
    Uxtx,
    /// Signed byte.
    Sxtb,
    /// Signed halfword.
    Sxth,
    /// Signed word.
    Sxtw,
    /// Signed doubleword.
    Sxtx,
}
/// A load/store memory operand.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum MemOperand {
    /// Base plus scaled unsigned offset.
    Unsigned {
        base: RegOrSp,
        offset: u16,
        scale: u8,
    },
    /// Signed unscaled offset.
    Unscaled { base: RegOrSp, offset: i16 },
    /// Pre-indexed signed offset.
    PreIndex { base: RegOrSp, offset: i16 },
    /// Post-indexed signed offset.
    PostIndex { base: RegOrSp, offset: i16 },
    /// Register offset.
    Register {
        base: RegOrSp,
        index: Reg,
        extend: Option<Extend>,
        shift: u8,
    },
}
/// A closed set of instructions used by the native backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Inst {
    /// Move wide immediate.
    MovZ { rd: Reg, imm: u16, shift: u8 },
    /// Move wide keep.
    MovK { rd: Reg, imm: u16, shift: u8 },
    /// Move wide negate.
    MovN { rd: Reg, imm: u16, shift: u8 },
    /// Register move.
    Mov { rd: RegOrSp, rn: RegOrSp },
    /// Add immediate.
    AddImm {
        rd: RegOrSp,
        rn: RegOrSp,
        imm: u16,
        shift: bool,
    },
    /// Sub immediate.
    SubImm {
        rd: RegOrSp,
        rn: RegOrSp,
        imm: u16,
        shift: bool,
    },
    /// Add shifted register.
    Add {
        rd: RegOrSp,
        rn: RegOrSp,
        rm: Reg,
        shift: Shift,
    },
    /// Sub shifted register.
    Sub {
        rd: RegOrSp,
        rn: RegOrSp,
        rm: Reg,
        shift: Shift,
    },
    /// Load/store 64-bit.
    Ldr { rt: Reg, mem: MemOperand },
    /// Store 64-bit.
    Str { rt: Reg, mem: MemOperand },
    /// Load/store 32-bit.
    LdrW { rt: Reg, mem: MemOperand },
    /// Store 32-bit.
    StrW { rt: Reg, mem: MemOperand },
    /// Branch.
    B { label: Label },
    /// Branch with link.
    Bl { label: Label },
    /// Conditional branch.
    BCond { cond: Cond, label: Label },
    /// Return.
    Ret { rn: Reg },
    /// Indirect branch with link.
    Blr { rn: Reg },
    /// Indirect branch.
    Br { rn: Reg },
    /// Compare and branch zero.
    Cbz { rt: Reg, label: Label },
    /// Compare and branch nonzero.
    Cbnz { rt: Reg, label: Label },
    /// PC-relative address.
    Adr { rd: Reg, label: Label },
    /// Page PC-relative address.
    Adrp { rd: Reg, label: Label },
    /// No operation.
    Nop,
    /// Breakpoint.
    Brk { imm: u16 },
    /// Add/sub aliases.
    Cmp { rn: Reg, rm: Reg, shift: Shift },
    /// Select.
    Csel {
        rd: Reg,
        rn: Reg,
        rm: Reg,
        cond: Cond,
    },
    /// Multiply.
    Mul { rd: Reg, rn: Reg, rm: Reg },
    /// Signed divide.
    Sdiv { rd: Reg, rn: Reg, rm: Reg },
    /// Unsigned divide.
    Udiv { rd: Reg, rn: Reg, rm: Reg },
    /// Logical shifted register.
    And {
        rd: Reg,
        rn: Reg,
        rm: Reg,
        shift: Shift,
    },
    /// Logical OR.
    Orr {
        rd: Reg,
        rn: Reg,
        rm: Reg,
        shift: Shift,
    },
    /// Logical XOR.
    Eor {
        rd: Reg,
        rn: Reg,
        rm: Reg,
        shift: Shift,
    },
    /// Test bits.
    Tst { rn: Reg, rm: Reg, shift: Shift },
    /// Undefined instruction.
    Udf { imm: u16 },
    /// Data memory barrier.
    DmbIsh,
}

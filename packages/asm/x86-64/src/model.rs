use core::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Reg {
    Rax = 0,
    Rdx = 1,
    Rdi = 2,
    Rsi = 3,
    Rcx = 4,
    R8 = 5,
    R9 = 6,
    R10 = 7,
    R11 = 8,
    Rbp = 9,
    Rbx = 10,
    R12 = 11,
    R13 = 12,
    R14 = 13,
    R15 = 14,
    Rsp = 15,
}
impl Reg {
    pub const fn id(self) -> u8 {
        self as u8
    }
    pub const fn code(self) -> u8 {
        [0, 2, 7, 6, 1, 8, 9, 10, 11, 5, 3, 12, 13, 14, 15, 4][self.id() as usize]
    }
    pub const fn from_id(id: u8) -> Option<Self> {
        match id {
            0 => Some(Self::Rax),
            1 => Some(Self::Rdx),
            2 => Some(Self::Rdi),
            3 => Some(Self::Rsi),
            4 => Some(Self::Rcx),
            5 => Some(Self::R8),
            6 => Some(Self::R9),
            7 => Some(Self::R10),
            8 => Some(Self::R11),
            9 => Some(Self::Rbp),
            10 => Some(Self::Rbx),
            11 => Some(Self::R12),
            12 => Some(Self::R13),
            13 => Some(Self::R14),
            14 => Some(Self::R15),
            15 => Some(Self::Rsp),
            _ => None,
        }
    }
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Rax),
            1 => Some(Self::Rcx),
            2 => Some(Self::Rdx),
            3 => Some(Self::Rbx),
            4 => Some(Self::Rsp),
            5 => Some(Self::Rbp),
            6 => Some(Self::Rsi),
            7 => Some(Self::Rdi),
            8 => Some(Self::R8),
            9 => Some(Self::R9),
            10 => Some(Self::R10),
            11 => Some(Self::R11),
            12 => Some(Self::R12),
            13 => Some(Self::R13),
            14 => Some(Self::R14),
            15 => Some(Self::R15),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Xmm(pub u8);
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Scale {
    One,
    Two,
    Four,
    Eight,
}
impl Scale {
    pub const fn bits(self) -> u8 {
        match self {
            Self::One => 0,
            Self::Two => 1,
            Self::Four => 2,
            Self::Eight => 3,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Mem {
    pub base: Option<Reg>,
    pub index: Option<Reg>,
    pub scale: Scale,
    pub disp: i32,
    pub rip: bool,
}
impl Mem {
    pub const fn base(base: Reg, disp: i32) -> Self {
        Self {
            base: Some(base),
            index: None,
            scale: Scale::One,
            disp,
            rip: false,
        }
    }
    pub const fn indexed(base: Reg, index: Reg, scale: Scale, disp: i32) -> Self {
        Self {
            base: Some(base),
            index: Some(index),
            scale,
            disp,
            rip: false,
        }
    }
    pub const fn rip(disp: i32) -> Self {
        Self {
            base: None,
            index: None,
            scale: Scale::One,
            disp,
            rip: true,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Imm {
    I8(i8),
    I32(i32),
    I64(i64),
}
impl Imm {
    pub const fn value(self) -> i64 {
        match self {
            Self::I8(v) => v as i64,
            Self::I32(v) => v as i64,
            Self::I64(v) => v,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Label(pub u32);
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Cond {
    O,
    No,
    B,
    Ae,
    E,
    Ne,
    Be,
    A,
    S,
    Ns,
    P,
    Np,
    L,
    Ge,
    Le,
    G,
}
impl Cond {
    pub const fn code(self) -> u8 {
        match self {
            Self::O => 0,
            Self::No => 1,
            Self::B => 2,
            Self::Ae => 3,
            Self::E => 4,
            Self::Ne => 5,
            Self::Be => 6,
            Self::A => 7,
            Self::S => 8,
            Self::Ns => 9,
            Self::P => 10,
            Self::Np => 11,
            Self::L => 12,
            Self::Ge => 13,
            Self::Le => 14,
            Self::G => 15,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    And,
    Or,
    Xor,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Shift {
    Shl,
    Shr,
    Sar,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SseOp {
    Addsd,
    Subsd,
    Mulsd,
    Divsd,
    Ucomisd,
    Sqrtsd,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Inst {
    MovRR(Reg, Reg),
    MovRI(Reg, Imm),
    MovRM(Reg, Mem),
    MovMR(Mem, Reg),
    MovMI(Mem, i32),
    Movzx(Reg, Reg, u8),
    MovzxRM(Reg, Mem, u8),
    Movsx(Reg, Reg, u8),
    MovsxRM(Reg, Mem, u8),
    Lea(Reg, Mem),
    BinRR(BinOp, Reg, Reg),
    BinRI(BinOp, Reg, i32),
    BinRM(BinOp, Reg, Mem),
    BinMR(BinOp, Mem, Reg),
    CmpRR(Reg, Reg),
    CmpRI(Reg, i32),
    CmpRM(Reg, Mem),
    CmpMR(Mem, Reg),
    TestRR(Reg, Reg),
    TestRM(Reg, Mem),
    TestMR(Mem, Reg),
    ImulRR(Reg, Reg),
    ImulRRI(Reg, Reg, i32),
    Neg(Reg),
    Not(Reg),
    ShiftImm(Shift, Reg, u8),
    ShiftCl(Shift, Reg),
    Inc(Reg),
    Dec(Reg),
    Cqo,
    Idiv(Reg),
    Setcc(Cond, Reg),
    Cmovcc(Cond, Reg, Reg),
    Jmp(Label),
    JmpReg(Reg),
    JmpMem(Mem),
    Jcc(Cond, Label),
    Call(Label),
    CallReg(Reg),
    CallMem(Mem),
    Ret,
    Push(Reg),
    Pop(Reg),
    Nop(u8),
    Ud2,
    Int3,
    Xchg(Reg, Reg),
    Bt(Reg, u8),
    MovsdRM(Xmm, Mem),
    MovsdMR(Mem, Xmm),
    MovqXR(Xmm, Reg),
    MovqRX(Reg, Xmm),
    Sse(SseOp, Xmm, Xmm),
    SseRM(SseOp, Xmm, Mem),
    Cvtsi2sd(Xmm, Reg),
    Cvtsi2sdRM(Xmm, Mem),
    Cvttsd2si(Reg, Xmm),
    Cvttsd2siRM(Reg, Mem),
    Xorpd(Xmm, Xmm),
}
impl fmt::Display for Reg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Rax => "rax",
            Self::Rdx => "rdx",
            Self::Rdi => "rdi",
            Self::Rsi => "rsi",
            Self::Rcx => "rcx",
            Self::R8 => "r8",
            Self::R9 => "r9",
            Self::R10 => "r10",
            Self::R11 => "r11",
            Self::Rbp => "rbp",
            Self::Rbx => "rbx",
            Self::R12 => "r12",
            Self::R13 => "r13",
            Self::R14 => "r14",
            Self::R15 => "r15",
            Self::Rsp => "rsp",
        })
    }
}

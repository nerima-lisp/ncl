//! The value objects and instruction vocabulary of the IR.
#![allow(missing_docs)]
#![allow(clippy::all)]

use std::fmt::{self, Display, Formatter};

macro_rules! id_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(pub u32);
        impl Display for $name { fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result { write!(f, "%{}", self.0) } }
    };
}

id_type!(
    #[doc = "A function identity."]
    FunctionId
);
id_type!(
    #[doc = "A basic-block identity."]
    BlockId
);
id_type!(
    #[doc = "An SSA value identity."]
    ValueId
);
id_type!(
    #[doc = "A local binding identity."]
    LocalId
);
id_type!(
    #[doc = "A constant-table index."]
    ConstantIndex
);
id_type!(
    #[doc = "A source-file identity."]
    FileId
);
id_type!(
    #[doc = "A source form identity."]
    FormId
);
id_type!(
    #[doc = "A handler-region identity."]
    HandlerRegionId
);
id_type!(
    #[doc = "A debug-location identity."]
    DebugLocationId
);

/// A machine-independent IR scalar type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ty {
    Word,
    I64,
    F64,
    Address,
    Bool,
    Unit,
}
impl Display for Ty {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Word => "word",
                Self::I64 => "i64",
                Self::F64 => "f64",
                Self::Address => "address",
                Self::Bool => "bool",
                Self::Unit => "unit",
            }
        )
    }
}

/// A parameter of a function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Ty,
}
/// A local binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Local {
    pub id: LocalId,
    pub name: String,
    pub ty: Ty,
}
/// A block parameter, which represents an SSA phi value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockParam {
    pub value: ValueId,
    pub ty: Ty,
}
/// A source location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugLocation {
    pub file: FileId,
    pub line: u32,
    pub column: u32,
    pub form: FormId,
}

/// The shape of a frozen heap object described by a constant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StructureKind {
    Cons,
    SimpleVector,
}

/// A constant descriptor resolved by the runtime or code generator.
#[derive(Clone, Debug, PartialEq)]
pub enum Constant {
    Fixnum(i64),
    Character(u32),
    SingleFloat(f32),
    DoubleFloat(f64),
    Symbol {
        package: String,
        name: String,
    },
    UninternedSymbol {
        identity: u32,
        name: String,
    },
    Object(ConstantIndex),
    StringBytes(Vec<u8>),
    Structure {
        kind: StructureKind,
        elements: Vec<ConstantIndex>,
    },
    Nil,
    T,
    Unbound,
    FunctionEntry(FunctionId),
    /// An integer wider than a fixnum, as a sign plus little-endian 32-bit limbs.
    Bignum {
        negative: bool,
        limbs: Vec<u32>,
    },
    /// A ratio; both operands are resolved constants earlier in the table.
    Ratio {
        numerator: ConstantIndex,
        denominator: ConstantIndex,
    },
    /// A complex number; both parts are resolved constants earlier in the table.
    Complex {
        real: ConstantIndex,
        imaginary: ConstantIndex,
    },
}

/// A direct primitive operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Prim {
    Car,
    Cdr,
    Rplaca,
    Rplacd,
    Svref,
    Aref,
    Aset,
    FixnumAdd,
    FixnumSub,
    FixnumMul,
    FixnumDiv,
    FixnumLt,
    FixnumLe,
    FixnumEq,
    Eq,
    Eql,
    Typep,
    CharacterPredicate(String),
    StructureSlot(String),
}

/// A checked fixnum operation attached to a generic builtin call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixnumOp {
    Add,
    Sub,
    Mul,
    LessThan,
    OnePlus,
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
            _ => return None,
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
            _ => None,
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

impl Prim {
    #[must_use]
    pub const fn is_pure(&self) -> bool {
        !matches!(self, Self::Rplaca | Self::Rplacd | Self::Aset)
    }

    #[must_use]
    pub const fn reads_memory(&self) -> bool {
        matches!(
            self,
            Self::Car | Self::Cdr | Self::Svref | Self::Aref | Self::StructureSlot(_)
        )
    }

    #[must_use]
    pub const fn invalidates_memory(&self) -> bool {
        matches!(self, Self::Rplaca | Self::Rplacd | Self::Aset)
    }
}
/// A comparison operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Compare {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}
/// A conversion operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Convert {
    WordToI64,
    I64ToWord,
    WordToF64,
    F64ToWord,
    AddressToWord,
    WordToAddress,
}

/// An operation with SSA results and optional source location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Op {
    pub results: Vec<(ValueId, Ty)>,
    pub kind: OpKind,
    pub loc: Option<DebugLocationId>,
}
/// An operation in a basic block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpKind {
    Const {
        result: ConstantIndex,
    },
    Move {
        value: ValueId,
    },
    Load {
        address: ValueId,
    },
    Store {
        address: ValueId,
        value: ValueId,
    },
    LoadField {
        object: ValueId,
        field: u32,
    },
    StoreField {
        object: ValueId,
        field: u32,
        value: ValueId,
    },
    Alloc {
        words: u32,
    },
    LoadArg {
        index: u8,
    },
    LoadCapture {
        index: u8,
    },
    LoadFunctionObject,
    Call {
        function: ValueId,
        args: Vec<ValueId>,
    },
    CallIndirect {
        callee: ValueId,
        args: Vec<ValueId>,
    },
    MakeClosure {
        entry: ValueId,
        captures: Vec<ValueId>,
    },
    MakeValueCell {
        value: ValueId,
    },
    CallClosure {
        closure: ValueId,
        args: Vec<ValueId>,
        named_symbol: Option<ValueId>,
    },
    Builtin {
        name: String,
        args: Vec<ValueId>,
        specialization: Option<FixnumSpecialization>,
    },
    Prim {
        op: Prim,
        args: Vec<ValueId>,
        condition: Option<BlockId>,
    },
    Compare {
        op: Compare,
        left: ValueId,
        right: ValueId,
    },
    Convert {
        op: Convert,
        value: ValueId,
    },
    SetMultipleValues {
        values: Vec<ValueId>,
    },
    Safepoint,
    EnterHandler {
        region: HandlerRegionId,
    },
    LeaveHandler {
        region: HandlerRegionId,
    },
}

/// A terminator and its successor arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Terminator {
    Jump {
        target: BlockId,
        args: Vec<ValueId>,
    },
    Branch {
        condition: ValueId,
        then_target: BlockId,
        then_args: Vec<ValueId>,
        else_target: BlockId,
        else_args: Vec<ValueId>,
    },
    Switch {
        value: ValueId,
        cases: Vec<(i64, BlockId, Vec<ValueId>)>,
        default: BlockId,
        default_args: Vec<ValueId>,
    },
    CallReturn {
        function: ValueId,
        args: Vec<ValueId>,
    },
    TailCall {
        function: ValueId,
        args: Vec<ValueId>,
    },
    Return {
        values: Vec<ValueId>,
    },
    Throw {
        condition: ValueId,
    },
    Unreachable,
}
/// An exception handler region.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandlerRegion {
    pub id: HandlerRegionId,
    pub kind: HandlerKind,
    pub protected: Vec<BlockId>,
    pub handler: BlockId,
    pub cleanup: Option<BlockId>,
    pub catch_tag: Option<ValueId>,
    pub binding_targets: Vec<ValueId>,
    pub depth: u32,
    pub parent: Option<HandlerRegionId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandlerKind {
    Catch,
    UnwindProtect,
    Progv,
}
/// A basic block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BasicBlock {
    pub id: BlockId,
    pub params: Vec<BlockParam>,
    pub ops: Vec<Op>,
    pub terminator: Terminator,
}
/// A complete compiler function.
#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub id: FunctionId,
    pub name: String,
    pub params: Vec<Param>,
    pub return_types: Vec<Ty>,
    pub blocks: Vec<BasicBlock>,
    pub locals: Vec<Local>,
    pub constants: Vec<Constant>,
    pub handler_regions: Vec<HandlerRegion>,
    pub debug: Vec<DebugLocation>,
}

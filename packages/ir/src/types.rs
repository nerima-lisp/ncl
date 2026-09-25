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

/// A constant descriptor resolved by the runtime or code generator.
#[derive(Clone, Debug, PartialEq)]
pub enum Constant {
    Fixnum(i64),
    Character(u32),
    SingleFloat(f32),
    DoubleFloat(f64),
    Symbol { package: String, name: String },
    Object(ConstantIndex),
    StringBytes(Vec<u8>),
    Nil,
    T,
    Unbound,
    FunctionEntry(FunctionId),
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
    CallClosure {
        closure: ValueId,
        args: Vec<ValueId>,
    },
    Builtin {
        name: String,
        args: Vec<ValueId>,
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

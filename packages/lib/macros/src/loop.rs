//! Typed parser and structural expander for the ANSI CL LOOP facility.
//!
//! The registration glue intentionally lives outside this file.  The public
//! entry points here are suitable for the same adapted callback used by the
//! other macro expanders in this crate.

use crate::{elements, fresh_symbol, list, symbol};
use ncl_object::{
    MultipleValues, ObjectError, Runtime, ThreadContext, Word, string_length, string_ref,
    symbol_name,
};

type Result<T = Word> = std::result::Result<T, ObjectError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccumulatorKind {
    Collect,
    Append,
    Nconc,
    Count,
    Sum,
    Maximize,
    Minimize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepDirection {
    From,
    UpFrom,
    DownFrom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitDirection {
    To,
    UpTo,
    Below,
    DownTo,
    Above,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForClause {
    pub variable: Word,
    pub init: Word,
    pub step: Option<Word>,
    pub direction: Option<StepDirection>,
    pub limit: Option<(LimitDirection, Word)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HashIterationKind {
    Key,
    Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HashClause {
    pub variable: Word,
    pub kind: HashIterationKind,
    pub table: Word,
    pub using: Option<(HashIterationKind, Word)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoopClause {
    With {
        variable: Word,
        init: Word,
    },
    For(ForClause),
    Hash(HashClause),
    EqualsThen {
        variable: Word,
        init: Word,
        then: Word,
    },
    In {
        variable: Word,
        sequence: Word,
        on: bool,
        by: Option<Word>,
    },
    Across {
        variable: Word,
        vector: Word,
    },
    Repeat(Word),
    While(Word),
    Until(Word),
    Initially(Vec<Word>),
    Finally(Vec<Word>),
    Do(Vec<Word>),
    Accumulate {
        kind: AccumulatorKind,
        form: Word,
        variable: Option<Word>,
    },
    Return(Word),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoopAst {
    pub name: Option<Word>,
    pub clauses: Vec<LoopClause>,
}

mod accumulator;
mod callback;
mod clause;
mod expansion;
mod hash;
mod held;
mod parser;
pub use callback::expand_loop_callback;
#[allow(unused_imports)]
pub use expansion::expand_loop_ast;
pub use parser::parse_loop;

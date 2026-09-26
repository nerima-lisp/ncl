//! Typed parser and structural expander for the ANSI CL LOOP facility.
//!
//! The registration glue intentionally lives outside this file.  The public
//! entry points here are suitable for the same adapted callback used by the
//! other macro expanders in this crate.

use crate::{elements, fresh_symbol, list, symbol};
use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, Runtime, ThreadContext, Word, string_length,
    string_ref, symbol_name,
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForClause {
    pub variable: Word,
    pub init: Word,
    pub step: Option<Word>,
    pub direction: Option<StepDirection>,
    pub limit: Option<(LimitDirection, Word)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoopClause {
    With {
        variable: Word,
        init: Word,
    },
    For(ForClause),
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

mod expansion;
mod parser;
#[allow(unused_imports)]
pub use expansion::{expand_loop_ast, expand_loop_callback};
pub use parser::parse_loop;

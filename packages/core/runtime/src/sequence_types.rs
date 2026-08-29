#![allow(clippy::wildcard_imports)]

use super::*;

#[derive(Clone, Copy)]
pub(super) enum SequenceKind {
    List,
    Vector,
    String,
}

#[derive(Clone, Copy)]
pub(super) struct SequenceMergeContext<'a> {
    pub(super) result_type: &'a Value,
    pub(super) sequence1: &'a Value,
    pub(super) sequence2: &'a Value,
    pub(super) predicate: &'a Value,
    pub(super) options: &'a [Value],
    pub(super) environment: &'a Environment,
    pub(super) span: Span,
}

#[derive(Clone, Copy)]
pub(super) struct SequenceSubstituteContext<'a> {
    pub(super) operation: &'a str,
    pub(super) new_item: &'a Value,
    pub(super) old_or_predicate: &'a Value,
    pub(super) sequence: &'a Value,
    pub(super) options: &'a [Value],
    pub(super) environment: &'a Environment,
    pub(super) span: Span,
}

pub(super) struct SequenceSubstituteOptions {
    pub(super) from_end: bool,
    pub(super) test: Option<Value>,
    pub(super) test_not: Option<Value>,
    pub(super) key: Option<Value>,
    pub(super) start: usize,
    pub(super) end: Option<usize>,
    pub(super) count: Option<usize>,
}

#[derive(Clone, Copy)]
pub(super) struct SequenceSubstituteMatchContext<'a> {
    pub(super) items: &'a [Value],
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) key_function: &'a Option<std::rc::Rc<crate::Function>>,
    pub(super) test_function: &'a Value,
    pub(super) old_or_predicate: &'a Value,
    pub(super) is_predicate: bool,
    pub(super) invert_test: bool,
    pub(super) environment: &'a Environment,
    pub(super) span: Span,
}

#[derive(Clone, Copy)]
pub(super) struct SequencePairSearchOperationContext<'a> {
    pub(super) items1: &'a [Value],
    pub(super) items2: &'a [Value],
    pub(super) start1: usize,
    pub(super) end1: usize,
    pub(super) start2: usize,
    pub(super) end2: usize,
}

pub(super) struct ListMembershipOptions {
    pub(super) test: Option<Value>,
    pub(super) test_not: Option<Value>,
    pub(super) key: Option<Value>,
}

pub(super) struct AssociationSearchOptions {
    pub(super) test: Option<Value>,
    pub(super) test_not: Option<Value>,
    pub(super) key: Option<Value>,
}

pub(super) struct SequenceReduceOptions {
    pub(super) from_end: bool,
    pub(super) start: usize,
    pub(super) end: Option<usize>,
    pub(super) initial_value: Option<Value>,
    pub(super) key: Option<Value>,
}

pub(super) struct SequenceSearchOptions {
    pub(super) from_end: bool,
    pub(super) test: Option<Value>,
    pub(super) test_not: Option<Value>,
    pub(super) key: Option<Value>,
    pub(super) start: usize,
    pub(super) end: Option<usize>,
}

pub(super) struct SequencePairSearchOptions {
    pub(super) from_end: bool,
    pub(super) test: Option<Value>,
    pub(super) test_not: Option<Value>,
    pub(super) key: Option<Value>,
    pub(super) start1: usize,
    pub(super) end1: Option<usize>,
    pub(super) start2: usize,
    pub(super) end2: Option<usize>,
}

#[derive(Clone)]
pub(super) struct SequenceRemoveOptions {
    pub(super) from_end: bool,
    pub(super) test: Option<Value>,
    pub(super) test_not: Option<Value>,
    pub(super) key: Option<Value>,
    pub(super) start: usize,
    pub(super) end: Option<usize>,
    pub(super) count: Option<usize>,
}

pub(super) struct SequenceRemovalContext<'a> {
    pub(super) items: &'a [Value],
    pub(super) candidates: &'a [Value],
    pub(super) end: usize,
    pub(super) options: &'a SequenceRemoveOptions,
    pub(super) item_or_predicate: &'a Value,
    pub(super) test_function: &'a Value,
    pub(super) is_predicate: bool,
    pub(super) removes_duplicates: bool,
    pub(super) invert_test: bool,
    pub(super) environment: &'a Environment,
    pub(super) span: Span,
}

pub(super) struct ListSetOptions {
    pub(super) test: Option<Value>,
    pub(super) test_not: Option<Value>,
    pub(super) key: Option<Value>,
}

pub(super) struct ListSetContext<'a> {
    pub(super) operation: &'a str,
    pub(super) first_items: &'a [Value],
    pub(super) second_items: &'a [Value],
    pub(super) first_keys: &'a [Value],
    pub(super) second_keys: &'a [Value],
    pub(super) test_function: &'a Value,
    pub(super) invert_test: bool,
    pub(super) environment: &'a Environment,
    pub(super) span: Span,
}

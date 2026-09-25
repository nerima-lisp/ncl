#![allow(missing_docs, clippy::missing_errors_doc)]

use ncl_object::{ObjectError, Runtime, ThreadContext, Word};
use std::collections::HashMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetfExpansion {
    pub temporary_variables: Vec<Word>,
    pub value_forms: Vec<Word>,
    pub store_variables: Vec<Word>,
    pub store_form: Word,
    pub access_form: Word,
}

pub type PlaceExpander =
    fn(&mut ThreadContext, &Runtime, &[Word]) -> Result<SetfExpansion, ObjectError>;

#[derive(Debug, Default)]
pub struct PlaceRegistry {
    expanders: HashMap<Word, PlaceExpander>,
}

impl PlaceRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn define(&mut self, operator: Word, expander: PlaceExpander) {
        self.expanders.insert(operator, expander);
    }
    pub fn remove(&mut self, operator: Word) -> Option<PlaceExpander> {
        self.expanders.remove(&operator)
    }
    #[must_use]
    pub fn get(&self, operator: Word) -> Option<PlaceExpander> {
        self.expanders.get(&operator).copied()
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.expanders.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.expanders.is_empty()
    }
}

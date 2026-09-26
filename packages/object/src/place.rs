//! Runtime-owned generalized-reference expanders.

use crate::runtime::RootedWord;
use crate::{ObjectError, Runtime, ThreadContext, Word, symbol_name};
use ncl_sys::Heap;

/// The five values returned by `get-setf-expansion`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetfExpansion {
    /// Variables receiving evaluated place subforms.
    pub temporary_variables: Vec<Word>,
    /// Place subforms evaluated for the temporary variables.
    pub value_forms: Vec<Word>,
    /// Variables receiving values to store.
    pub store_variables: Vec<Word>,
    /// Form that performs the store.
    pub store_form: Word,
    /// Form that reads the current place value.
    pub access_form: Word,
}

/// Callback used to expand a registered compound place.
pub type PlaceExpander =
    fn(&mut ThreadContext, &Runtime, &[Word]) -> Result<SetfExpansion, ObjectError>;

#[derive(Debug, Default)]
pub(crate) struct PlaceExpanders {
    entries: Vec<PlaceExpanderEntry>,
}

#[derive(Debug)]
struct PlaceExpanderEntry {
    operator: RootedWord,
    expander: PlaceExpander,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SymbolId(Word);

impl SymbolId {
    pub(crate) fn from_symbol(ctx: &ThreadContext, symbol: Word) -> Result<Self, ObjectError> {
        symbol_name(ctx, symbol)?;
        Ok(Self(symbol))
    }
}

impl PlaceExpanders {
    pub(crate) fn register(&mut self, heap: &Heap, operator: SymbolId, expander: PlaceExpander) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.operator.get() == operator.0)
        {
            entry.expander = expander;
            return;
        }

        self.entries.push(PlaceExpanderEntry {
            operator: RootedWord::new(heap, operator.0),
            expander,
        });
    }

    pub(crate) fn get(&self, operator: SymbolId) -> Option<PlaceExpander> {
        self.entries
            .iter()
            .find(|entry| entry.operator.get() == operator.0)
            .map(|entry| entry.expander)
    }
}

//! Runtime-owned generalized-reference expanders.

use crate::{ObjectError, Runtime, ThreadContext, Word, symbol_name};
use std::collections::HashMap;

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
    entries: HashMap<SymbolId, PlaceExpander>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct SymbolId(Word);

impl SymbolId {
    fn from_symbol(ctx: &ThreadContext, symbol: Word) -> Result<Self, ObjectError> {
        symbol_name(ctx, symbol)?;
        Ok(Self(symbol))
    }
}

impl PlaceExpanders {
    pub(crate) fn register(
        &mut self,
        ctx: &ThreadContext,
        operator: Word,
        expander: PlaceExpander,
    ) -> Result<(), ObjectError> {
        let id = SymbolId::from_symbol(ctx, operator)?;
        self.entries.insert(id, expander);
        Ok(())
    }

    pub(crate) fn get(
        &self,
        ctx: &ThreadContext,
        operator: Word,
    ) -> Result<Option<PlaceExpander>, ObjectError> {
        let id = SymbolId::from_symbol(ctx, operator)?;
        Ok(self.entries.get(&id).copied())
    }
}

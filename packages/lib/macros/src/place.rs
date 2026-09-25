#![allow(missing_docs, clippy::missing_errors_doc)]

use ncl_object::{ObjectError, Runtime, ThreadContext, Word};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

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

static RUNTIME_REGISTRIES: OnceLock<Mutex<HashMap<usize, PlaceRegistry>>> = OnceLock::new();

fn lock_registries() -> std::sync::MutexGuard<'static, HashMap<usize, PlaceRegistry>> {
    match runtime_registries().lock() {
        Ok(registries) => registries,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn runtime_registries() -> &'static Mutex<HashMap<usize, PlaceRegistry>> {
    RUNTIME_REGISTRIES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn runtime_key(runtime: &Runtime) -> usize {
    std::ptr::from_ref(runtime) as usize
}

/// Register a place expander in the registry owned by `runtime`.
pub fn register_place(runtime: &Runtime, operator: Word, expander: PlaceExpander) {
    let mut registries = lock_registries();
    registries
        .entry(runtime_key(runtime))
        .or_default()
        .define(operator, expander);
}

pub(crate) fn initialize_runtime_registry(runtime: &Runtime) {
    let mut registries = lock_registries();
    registries.entry(runtime_key(runtime)).or_default();
}

pub(crate) fn with_runtime_registry<T>(
    runtime: &Runtime,
    f: impl FnOnce(&PlaceRegistry) -> Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    let mut registries = lock_registries();
    let registry = registries.entry(runtime_key(runtime)).or_default();
    f(registry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expander(
        _ctx: &mut ThreadContext,
        _runtime: &Runtime,
        _arguments: &[Word],
    ) -> Result<SetfExpansion, ObjectError> {
        Err(ObjectError::TypeError)
    }

    #[test]
    fn runtime_registry_is_shared_and_runtime_scoped() -> Result<(), ObjectError> {
        let runtime = Runtime::new()?;
        let other_runtime = Runtime::new()?;
        let operator = Word::fixnum(7);

        initialize_runtime_registry(&runtime);
        assert_eq!(
            with_runtime_registry(&runtime, |registry| Ok(registry.len()))?,
            0
        );
        register_place(&runtime, operator, expander);
        assert_eq!(
            with_runtime_registry(&runtime, |registry| Ok(registry.len()))?,
            1
        );
        assert_eq!(
            with_runtime_registry(&other_runtime, |registry| {
                Ok(registry.get(operator).is_some())
            })?,
            false
        );
        assert!(with_runtime_registry(&runtime, |registry| {
            Ok(registry.get(operator).is_some())
        })?);
        Ok(())
    }
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

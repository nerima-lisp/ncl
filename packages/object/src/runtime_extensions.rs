use crate::hash_table::HashTable;
use crate::{ObjectError, Package, Runtime, ThreadContext, make_string};
use ncl_sys::{HeapConfig, StorageCondition, Word};

impl Runtime {
    /// Create a package if it does not already exist.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    pub fn ensure_package(&self, name: &str) -> Result<Word, ObjectError> {
        let mut context = self
            .registry_context
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut name_word = make_string(&mut context, self, &name.chars().collect::<Vec<_>>())?;
        let token = crate::push_root(&mut context, &mut name_word);
        let table = Self::table(&self.packages)?;
        if let Some(package) = HashTable::from(table).get(&mut context, name_word)? {
            let _ = crate::pop_root(&mut context, token);
            return Ok(package);
        }
        let mut package = Package::new(&mut context, self, name)?.as_word();
        let package_token = crate::push_root(&mut context, &mut package);
        let table = Self::table(&self.packages)?;
        HashTable::from(table).insert(&mut context, self, name_word, package)?;
        let _ = crate::pop_root(&mut context, package_token);
        let _ = crate::pop_root(&mut context, token);
        drop(context);
        Ok(package)
    }
    /// Find a package by its canonical name.
    #[must_use]
    pub fn find_package(&self, name: &str) -> Option<Word> {
        let mut context = self.registry_context.lock().ok()?;
        let name_word = make_string(&mut context, self, &name.chars().collect::<Vec<_>>()).ok()?;
        let table = Self::table(&self.packages).ok()?;
        let result = HashTable::from(table)
            .get(&mut context, name_word)
            .ok()
            .flatten();
        drop(context);
        result
    }
    /// Return the configured heap policy.
    #[must_use]
    pub const fn gc_config(&self) -> HeapConfig {
        HeapConfig {
            dynamic_space_size: self.heap().dynamic_space_size(),
            bytes_considered_between_gcs: self.heap().bytes_considered_between_gcs(),
        }
    }
    /// Register a class object by name.
    ///
    /// # Errors
    /// Returns an allocation, layout, or storage error.
    pub fn define_class(&self, name: impl Into<String>, class: Word) -> Result<(), ObjectError> {
        let mut context = self
            .registry_context
            .lock()
            .map_err(|_| ObjectError::Storage(StorageCondition::ThreadNotRegistered))?;
        let name = name.into();
        let mut name = make_string(&mut context, self, &name.chars().collect::<Vec<_>>())?;
        let name_token = crate::push_root(&mut context, &mut name);
        let mut class = class;
        let class_token = crate::push_root(&mut context, &mut class);
        let table = Self::table(&self.classes)?;
        let result = HashTable::from(table).insert(&mut context, self, name, class);
        let _ = crate::pop_root(&mut context, class_token);
        let _ = crate::pop_root(&mut context, name_token);
        drop(context);
        result
    }
    /// Look up a class object.
    #[must_use]
    pub fn class(&self, name: &str) -> Option<Word> {
        let mut context = self.registry_context.lock().ok()?;
        let name = make_string(&mut context, self, &name.chars().collect::<Vec<_>>()).ok()?;
        let table = Self::table(&self.classes).ok()?;
        let result = HashTable::from(table)
            .get(&mut context, name)
            .ok()
            .flatten();
        drop(context);
        result
    }
    /// Add a feature name if absent.
    pub fn add_feature(&self, feature: impl Into<String>) {
        let mut features = self
            .features
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let feature = feature.into();
        if !features.contains(&feature) {
            features.push(feature);
        }
    }
    /// Return the configured feature names.
    #[must_use]
    pub fn features(&self) -> Vec<String> {
        self.features
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl ThreadContext {
    /// Write a payload slot of a header object.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the object is unavailable to the registered thread.
    pub fn write_object_slot(
        &mut self,
        object: Word,
        slot: usize,
        value: Word,
    ) -> Result<(), ObjectError> {
        if ncl_sys::write_object_word(&mut self.thread, object, slot, value) {
            ncl_sys::write_barrier(&mut self.thread, object, slot);
            Ok(())
        } else {
            Err(ObjectError::Storage(StorageCondition::ThreadNotRegistered))
        }
    }
    /// Mark a pending non-local exit.
    pub const fn set_non_local_exit(&mut self, pending: bool) {
        self.non_local_exit = pending;
    }
    /// Return and clear the non-local exit flag.
    pub const fn take_non_local_exit(&mut self) -> bool {
        let pending = self.non_local_exit;
        self.non_local_exit = false;
        pending
    }
    /// Set the current handler, cleanup, and catch frame pointers.
    pub const fn set_control_pointers(
        &mut self,
        handler: Option<usize>,
        cleanup: Option<usize>,
        catch: Option<usize>,
    ) {
        self.handler = handler;
        self.cleanup = cleanup;
        self.catch = catch;
    }
    /// Return the current control frame pointers.
    #[must_use]
    pub const fn control_pointers(&self) -> (Option<usize>, Option<usize>, Option<usize>) {
        (self.handler, self.cleanup, self.catch)
    }
}

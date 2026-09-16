use crate::hash_table::HashTable;
use crate::{ObjectError, Package, Runtime, make_string};
use ncl_sys::{HeapConfig, StorageCondition, Word};

impl Runtime {
    pub fn set_gc_stress(&self, on: bool) {
        let mut context = self
            .registry_context
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        context.set_gc_stress(on);
    }

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
        let result = crate::with_root(&mut context, &mut name_word, |context, name_word| {
            let table = Self::table(&self.packages)?;
            if let Some(package) = HashTable::from(table).get(context, *name_word)? {
                return Ok(package);
            }
            let mut package = Package::new(context, self, name)?.as_word();
            crate::with_root(context, &mut package, |context, package| {
                HashTable::from(Self::table(&self.packages)?)
                    .insert(context, self, *name_word, *package)
            })?;
            Ok(package)
        });
        drop(context);
        result
    }

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

    #[must_use]
    pub const fn gc_config(&self) -> HeapConfig {
        HeapConfig {
            dynamic_space_size: self.heap.dynamic_space_size(),
            bytes_considered_between_gcs: self.heap.bytes_considered_between_gcs(),
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
        let mut class = class;
        let result = crate::with_root(&mut context, &mut class, |context, class| {
            let mut name = make_string(context, self, &name.chars().collect::<Vec<_>>())?;
            crate::with_root(context, &mut name, |context, name| {
                let table = Self::table(&self.classes)?;
                HashTable::from(table).insert(context, self, *name, *class)
            })
        });
        drop(context);
        result
    }

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

    #[must_use]
    pub fn features(&self) -> Vec<String> {
        self.features
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

use crate::hash_table::HashTable;
use crate::{ObjectError, Package, Runtime, ThreadContext, make_string, string_length, string_ref};
use ncl_sys::{HeapConfig, Word};

impl Runtime {
    /// Create a package if it does not already exist.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    pub fn ensure_package(&self, ctx: &mut ThreadContext, name: &str) -> Result<Word, ObjectError> {
        let mut name_word = make_string(ctx, self, &name.chars().collect::<Vec<_>>())?;
        crate::with_root(ctx, &mut name_word, |context, name_word| {
            let table = Self::table(&self.packages)?;
            if let Some(package) = HashTable::from_word(table).get(context, *name_word)? {
                return Ok(package);
            }
            let mut package = Package::new(context, self, name)?.as_word();
            crate::with_root(context, &mut package, |context, package| {
                HashTable::from_word(Self::table(&self.packages)?)
                    .insert(context, self, *name_word, *package)
            })?;
            Ok(package)
        })
    }

    #[must_use]
    pub fn find_package(&self, context: &ThreadContext, name: &str) -> Option<Word> {
        let table = Self::table(&self.packages).ok()?;
        let name_chars = name.chars().collect::<Vec<_>>();
        let mut result = None;
        let mut failure = None;
        HashTable::from_word(table)
            .for_each_entry(context, |_, package| {
                if result.is_some() || failure.is_some() {
                    return;
                }
                let package = match Package::try_from_word(context, package) {
                    Ok(package) => package,
                    Err(error) => {
                        failure = Some(error);
                        return;
                    }
                };
                let matches = |word: Word| {
                    string_length(context, word).ok() == Some(name_chars.len())
                        && name_chars
                            .iter()
                            .enumerate()
                            .all(|(i, c)| string_ref(context, word, i) == Ok(*c))
                };
                let package_name = match package.name(context) {
                    Ok(package_name) => package_name,
                    Err(error) => {
                        failure = Some(error);
                        return;
                    }
                };
                if matches(package_name) {
                    result = Some(package.as_word());
                    return;
                }
                let mut nicknames = match package.nicknames(context) {
                    Ok(nicknames) => nicknames,
                    Err(error) => {
                        failure = Some(error);
                        return;
                    }
                };
                while nicknames != Word::NIL {
                    let Some(nickname) = ncl_sys::read_cons_word(&context.thread, nicknames, 0)
                    else {
                        failure = Some(ObjectError::Layout);
                        return;
                    };
                    if matches(nickname) {
                        result = Some(package.as_word());
                        break;
                    }
                    let Some(next) = ncl_sys::read_cons_word(&context.thread, nicknames, 1) else {
                        failure = Some(ObjectError::Layout);
                        return;
                    };
                    nicknames = next;
                }
            })
            .ok()?;
        if failure.is_some() {
            return None;
        }
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
    pub fn define_class(
        &self,
        ctx: &mut ThreadContext,
        name: impl Into<String>,
        class: Word,
    ) -> Result<(), ObjectError> {
        let name = name.into();
        let mut class = class;
        crate::with_root(ctx, &mut class, |context, class| {
            let mut name = make_string(context, self, &name.chars().collect::<Vec<_>>())?;
            crate::with_root(context, &mut name, |context, name| {
                let table = Self::table(&self.classes)?;
                HashTable::from_word(table).insert(context, self, *name, *class)
            })
        })
    }

    #[must_use]
    pub fn class(&self, ctx: &mut ThreadContext, name: &str) -> Option<Word> {
        let name = make_string(ctx, self, &name.chars().collect::<Vec<_>>()).ok()?;
        let table = Self::table(&self.classes).ok()?;
        HashTable::from_word(table).get(ctx, name).ok().flatten()
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

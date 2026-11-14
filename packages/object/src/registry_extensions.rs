use crate::hash_table::HashTable;
use crate::{
    ObjectError, Package, Runtime, ThreadContext, make_string, string_length, string_ref,
    with_root, with_roots,
};
use ncl_sys::{Heap, HeapConfig, RootToken, Word};
use std::sync::Arc;

#[derive(Debug, Default)]
pub struct PackageNameIndex {
    entries: Vec<PackageNameEntry>,
}

#[derive(Debug)]
struct PackageNameEntry {
    name: String,
    package: RootedPackageWord,
    primary: bool,
}

#[derive(Debug)]
struct RootedPackageWord {
    heap: Arc<Heap>,
    slot: Option<Box<Word>>,
    token: RootToken,
}

impl RootedPackageWord {
    fn new(heap: &Arc<Heap>, value: Word) -> Self {
        let mut slot = Box::new(value);
        let token = ncl_sys::push_heap_root(heap, &mut slot);
        Self {
            heap: Arc::clone(heap),
            slot: Some(slot),
            token,
        }
    }

    fn get(&self) -> Word {
        self.slot.as_deref().copied().unwrap_or(Word::NIL)
    }
}

impl Drop for RootedPackageWord {
    fn drop(&mut self) {
        if self.heap.unregister_root(self.token).is_err()
            && let Some(slot) = self.slot.take()
        {
            // The root registry still holds this slot's address.
            std::mem::forget(slot);
        }
    }
}

impl PackageNameIndex {
    pub(crate) fn find(&self, name: &str, primary_only: bool) -> Option<Word> {
        let index = self
            .entries
            .binary_search_by(|entry| entry.name.as_str().cmp(name))
            .ok()?;
        let entry = self.entries.get(index)?;
        if primary_only && !entry.primary {
            return None;
        }
        Some(entry.package.get())
    }

    pub(crate) fn insert(
        &mut self,
        heap: &Arc<Heap>,
        name: String,
        package: Word,
        primary: bool,
    ) -> Result<(), ObjectError> {
        match self
            .entries
            .binary_search_by(|entry| entry.name.as_str().cmp(&name))
        {
            Ok(index) => {
                let entry = self.entries.get_mut(index).ok_or(ObjectError::Layout)?;
                if entry.package.get() != package {
                    return Err(ObjectError::PackageConflict);
                }
                entry.primary |= primary;
            }
            Err(index) => self.entries.insert(
                index,
                PackageNameEntry {
                    name,
                    package: RootedPackageWord::new(heap, package),
                    primary,
                },
            ),
        }
        Ok(())
    }

    pub(crate) fn remove(&mut self, name: &str) -> bool {
        let Ok(index) = self
            .entries
            .binary_search_by(|entry| entry.name.as_str().cmp(name))
        else {
            return false;
        };
        self.entries.remove(index);
        true
    }
}

fn string_text(ctx: &ThreadContext, string: Word) -> Result<String, ObjectError> {
    let mut text = String::new();
    for index in 0..string_length(ctx, string)? {
        text.push(string_ref(ctx, string, index)?);
    }
    Ok(text)
}

fn standard_package_name(name: &str) -> (&str, Option<&'static str>) {
    if name == "CL" || name == "COMMON-LISP" {
        ("COMMON-LISP", Some("CL"))
    } else if name == "CL-USER" || name == "COMMON-LISP-USER" {
        ("COMMON-LISP-USER", Some("CL-USER"))
    } else {
        (name, None)
    }
}

impl Runtime {
    pub(crate) fn package_for_name(&self, name: &str, primary_only: bool) -> Option<Word> {
        self.package_names
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .find(name, primary_only)
    }

    pub(crate) fn index_package_name(
        &self,
        name: String,
        package: Word,
        primary: bool,
    ) -> Result<(), ObjectError> {
        self.package_names
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(&self.heap, name, package, primary)
    }

    pub(crate) fn unindex_package_name(&self, name: &str) -> bool {
        self.package_names
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(name)
    }

    /// Return all packages currently registered in the runtime.
    ///
    /// # Errors
    /// Returns an object error when the package registry is malformed.
    pub fn all_packages(&self, ctx: &ThreadContext) -> Result<Vec<Word>, ObjectError> {
        let table = Self::table(&self.packages)?;
        let mut packages = Vec::new();
        HashTable::from_word(table).for_each_entry(ctx, |_, package| packages.push(package))?;
        Ok(packages)
    }

    /// Create a package if it does not already exist.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    pub fn ensure_package(&self, ctx: &mut ThreadContext, name: &str) -> Result<Word, ObjectError> {
        let (canonical_name, nickname) = standard_package_name(name);
        if canonical_name != name
            && let Some(package) = self.resolve_package(ctx, name)?
        {
            return Ok(package);
        }
        let existing = if nickname.is_some() {
            self.resolve_package_name(ctx, canonical_name)?
        } else {
            self.resolve_package(ctx, canonical_name)?
        };
        if let Some(mut package) = existing {
            if let Some(nickname) = nickname
                && self.resolve_package(ctx, nickname)?.is_none()
            {
                self.register_standard_nickname(ctx, &mut package, nickname)?;
            }
            return Ok(package);
        }
        if let Some(nickname) = nickname {
            let mut nickname_word = make_string(ctx, self, &nickname.chars().collect::<Vec<_>>())?;
            crate::with_root(ctx, &mut nickname_word, |ctx, nickname_word| {
                self.check_package_name(ctx, Word::NIL, *nickname_word)
            })?;
        }
        let mut name_word = make_string(ctx, self, &canonical_name.chars().collect::<Vec<_>>())?;
        crate::with_root(ctx, &mut name_word, |context, name_word| {
            let existing = if nickname.is_some() {
                self.resolve_package_name(context, canonical_name)?
            } else {
                self.resolve_package(context, canonical_name)?
            };
            if let Some(mut package) = existing {
                if let Some(nickname) = nickname
                    && self.resolve_package(context, nickname)?.is_none()
                {
                    self.register_standard_nickname(context, &mut package, nickname)?;
                }
                return Ok(package);
            }
            let mut package = Package::new(context, self, canonical_name)?.as_word();
            crate::with_root(context, &mut package, |context, package| {
                self.register_package_name(context, *package, *name_word)
            })?;
            if let Some(nickname) = nickname {
                self.register_standard_nickname(context, &mut package, nickname)?;
            }
            Ok(package)
        })
    }

    fn register_standard_nickname(
        &self,
        ctx: &mut ThreadContext,
        package: &mut Word,
        nickname: &str,
    ) -> Result<(), ObjectError> {
        crate::with_root(ctx, package, |ctx, package| {
            let mut nickname = make_string(ctx, self, &nickname.chars().collect::<Vec<_>>())?;
            crate::with_root(ctx, &mut nickname, |ctx, nickname| {
                self.check_package_name(ctx, *package, *nickname)?;
                Package::from_word(*package).register_nickname(ctx, self, *nickname)?;
                self.register_package_name(ctx, *package, *nickname)
            })
        })
    }

    /// Check whether a name is available in the runtime's package registry.
    ///
    /// # Errors
    /// Returns [`ObjectError::PackageConflict`] when another package owns the name.
    pub fn check_package_name(
        &self,
        ctx: &mut ThreadContext,
        package: Word,
        name: Word,
    ) -> Result<(), ObjectError> {
        if let Some(found) = HashTable::from_word(Self::table(&self.packages)?).get(ctx, name)?
            && found != package
        {
            return Err(ObjectError::PackageConflict);
        }
        Ok(())
    }

    /// Insert a package name in the runtime's rooted package table.
    ///
    /// # Errors
    /// Returns an object error if the name conflicts or the table is malformed.
    pub fn register_package_name(
        &self,
        ctx: &mut ThreadContext,
        package: Word,
        name: Word,
    ) -> Result<(), ObjectError> {
        let mut package = package;
        with_root(ctx, &mut package, |ctx, package| {
            let mut name = name;
            with_root(ctx, &mut name, |ctx, name| {
                self.check_package_name(ctx, *package, *name)?;
                let text = string_text(ctx, *name)?;
                let primary = string_text(ctx, Package::from_word(*package).name(ctx)?)? == text;
                if self
                    .package_for_name(&text, false)
                    .is_some_and(|registered| registered != *package)
                {
                    return Err(ObjectError::PackageConflict);
                }
                HashTable::from_word(Self::table(&self.packages)?)
                    .insert(ctx, self, *name, *package)?;
                self.index_package_name(text, *package, primary)
            })
        })
    }

    /// Remove one name from the runtime's rooted package table.
    ///
    /// # Errors
    /// Returns an object error if the table is malformed.
    pub fn unregister_package_name(
        &self,
        ctx: &mut ThreadContext,
        name: Word,
    ) -> Result<bool, ObjectError> {
        let mut name = name;
        with_root(ctx, &mut name, |ctx, name| {
            let removed = HashTable::from_word(Self::table(&self.packages)?)
                .remove(ctx, self, *name)?
                .is_some();
            if removed && *name != Word::NIL {
                self.unindex_package_name(&string_text(ctx, *name)?);
            }
            Ok(removed)
        })
    }

    /// Remove a registered package name only when `package` owns it.
    ///
    /// # Errors
    /// Returns an object error when the registry is malformed.
    pub fn unregister_package_name_for_package(
        &self,
        ctx: &mut ThreadContext,
        package: Word,
        name: Word,
    ) -> Result<bool, ObjectError> {
        let roots = [package, name];
        with_roots(ctx, &roots, |ctx, roots| {
            let table = HashTable::from_word(Self::table(&self.packages)?);
            let package = roots.first().ok_or(ObjectError::Layout)?;
            let name = roots.get(1).ok_or(ObjectError::Layout)?;
            if table.get(ctx, **name)? != Some(**package) {
                return Ok(false);
            }
            self.unregister_package_name(ctx, **name)
        })
    }

    /// Return whether `package` is registered under its primary name.
    ///
    /// # Errors
    /// Returns an object error when package metadata or the registry is malformed.
    pub fn package_is_registered(
        &self,
        ctx: &mut ThreadContext,
        package: Word,
    ) -> Result<bool, ObjectError> {
        let mut package = package;
        with_root(ctx, &mut package, |ctx, package| {
            let mut name = Package::from_word(*package).name(ctx)?;
            with_root(ctx, &mut name, |ctx, name| {
                let table = HashTable::from_word(Self::table(&self.packages)?);
                Ok(table.get(ctx, *name)? == Some(*package))
            })
        })
    }

    fn resolve_package(
        &self,
        context: &ThreadContext,
        name: &str,
    ) -> Result<Option<Word>, ObjectError> {
        if context.registered {
            Ok(self.package_for_name(name, false))
        } else {
            self.resolve_package_linearly(context, name, false)
        }
    }

    fn resolve_package_name(
        &self,
        context: &ThreadContext,
        name: &str,
    ) -> Result<Option<Word>, ObjectError> {
        if context.registered {
            Ok(self.package_for_name(name, true))
        } else {
            self.resolve_package_linearly(context, name, true)
        }
    }

    fn resolve_package_linearly(
        &self,
        context: &ThreadContext,
        name: &str,
        primary_only: bool,
    ) -> Result<Option<Word>, ObjectError> {
        let table = Self::table(&self.packages)?;
        let name_chars = name.chars().collect::<Vec<_>>();
        let mut result = None;
        let mut failure = None;
        HashTable::from_word(table).for_each_entry(context, |_, word| {
            if result.is_some() || failure.is_some() {
                return;
            }
            let package = match Package::try_from_word(context, word) {
                Ok(package) => package,
                Err(error) => {
                    failure = Some(error);
                    return;
                }
            };
            let matches = |word: Word| {
                string_length(context, word).ok() == Some(name_chars.len())
                    && name_chars.iter().enumerate().all(|(index, character)| {
                        string_ref(context, word, index) == Ok(*character)
                    })
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
            if primary_only {
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
                let Some(nickname) = ncl_sys::read_cons_word(&context.thread, nicknames, 0) else {
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
        })?;
        failure.map_or(Ok(result), Err)
    }

    #[must_use]
    pub fn find_package(&self, context: &ThreadContext, name: &str) -> Option<Word> {
        self.resolve_package(context, name).ok().flatten()
    }

    #[must_use]
    pub fn gc_config(&self) -> HeapConfig {
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
                let mut table = Self::table(&self.classes)?;
                crate::with_root(context, &mut table, |context, table| {
                    HashTable::from_word(*table).insert(context, self, *name, *class)
                })
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

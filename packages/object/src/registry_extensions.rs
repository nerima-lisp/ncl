use crate::{ObjectError, Package, Runtime, ThreadContext, make_string, string_length, string_ref};
use ncl_sys::{HeapConfig, StorageCondition, Word};

fn string_from_word(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    let length = string_length(ctx, word)?;
    let mut value = String::new();
    for index in 0..length {
        value.push(string_ref(ctx, word, index)?);
    }
    Ok(value)
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
    /// Return all packages currently registered in the runtime.
    ///
    /// # Errors
    /// Returns an object error when the package registry is malformed.
    pub fn all_packages(&self, _ctx: &ThreadContext) -> Result<Vec<Word>, ObjectError> {
        let packages = self
            .packages
            .lock()
            .map_err(|_| ObjectError::Storage(StorageCondition::ThreadNotRegistered))?;
        Ok(packages
            .values()
            .map(crate::runtime::RootedWord::get)
            .collect())
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
                self.register_package_name(context, *package, *name_word, false)
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
                self.register_package_name(ctx, *package, *nickname, true)
            })
        })
    }

    /// Check whether a package name is available for this package.
    ///
    /// # Errors
    /// Returns [`ObjectError::PackageConflict`] when another package owns the name, or an object
    /// error if the name is invalid or registry storage is unavailable.
    pub fn check_package_name(
        &self,
        ctx: &mut ThreadContext,
        package: Word,
        name: Word,
    ) -> Result<(), ObjectError> {
        let name = string_from_word(ctx, name)?;
        let packages = self
            .packages
            .lock()
            .map_err(|_| ObjectError::Storage(StorageCondition::ThreadNotRegistered))?;
        if let Some(found) = packages.get(&name)
            && found.get() != package
        {
            return Err(ObjectError::PackageConflict);
        }
        drop(packages);
        Ok(())
    }

    /// Register a package under its name and optionally as a nickname.
    ///
    /// # Errors
    /// Returns an object error if the name conflicts, the package metadata is malformed, or
    /// registry storage is unavailable.
    pub fn register_package_name(
        &self,
        ctx: &mut ThreadContext,
        package: Word,
        name: Word,
        nickname: bool,
    ) -> Result<(), ObjectError> {
        let mut package = package;
        let mut name = name;
        crate::with_root(ctx, &mut package, |ctx, package| {
            crate::with_root(ctx, &mut name, |ctx, name| {
                self.check_package_name(ctx, *package, *name)?;
                if nickname {
                    Package::from_word(*package).register_nickname(ctx, self, *name)?;
                }
                let key = string_from_word(ctx, *name)?;
                self.packages
                    .lock()
                    .map_err(|_| ObjectError::Storage(StorageCondition::ThreadNotRegistered))?
                    .insert(key, crate::runtime::RootedWord::new(&self.heap, *package));
                Ok(())
            })
        })
    }

    /// Remove a package name from the runtime registry.
    ///
    /// # Errors
    /// Returns an object error if the name is invalid or registry storage is unavailable.
    pub fn unregister_package_name(
        &self,
        ctx: &ThreadContext,
        name: Word,
    ) -> Result<bool, ObjectError> {
        let key = string_from_word(ctx, name)?;
        Ok(self
            .packages
            .lock()
            .map_err(|_| ObjectError::Storage(StorageCondition::ThreadNotRegistered))?
            .remove(&key)
            .is_some())
    }

    fn resolve_package(
        &self,
        context: &ThreadContext,
        name: &str,
    ) -> Result<Option<Word>, ObjectError> {
        let name_chars = name.chars().collect::<Vec<_>>();
        let packages = self
            .packages
            .lock()
            .map_err(|_| ObjectError::Storage(StorageCondition::ThreadNotRegistered))?;
        for rooted_package in packages.values() {
            let package = Package::try_from_word(context, rooted_package.get())?;
            let matches = |word: Word| {
                string_length(context, word).ok() == Some(name_chars.len())
                    && name_chars
                        .iter()
                        .enumerate()
                        .all(|(i, c)| string_ref(context, word, i) == Ok(*c))
            };
            let package_name = package.name(context)?;
            if matches(package_name) {
                return Ok(Some(package.as_word()));
            }
            let mut nicknames = package.nicknames(context)?;
            while nicknames != Word::NIL {
                let nickname = ncl_sys::read_cons_word(&context.thread, nicknames, 0)
                    .ok_or(ObjectError::Layout)?;
                if matches(nickname) {
                    return Ok(Some(package.as_word()));
                }
                nicknames = ncl_sys::read_cons_word(&context.thread, nicknames, 1)
                    .ok_or(ObjectError::Layout)?;
            }
        }
        drop(packages);
        Ok(None)
    }

    fn resolve_package_name(
        &self,
        context: &ThreadContext,
        name: &str,
    ) -> Result<Option<Word>, ObjectError> {
        let name_chars = name.chars().collect::<Vec<_>>();
        let packages = self
            .packages
            .lock()
            .map_err(|_| ObjectError::Storage(StorageCondition::ThreadNotRegistered))?;
        for rooted_package in packages.values() {
            let package = Package::try_from_word(context, rooted_package.get())?;
            let package_name = package.name(context)?;
            let matches = string_length(context, package_name).ok() == Some(name_chars.len())
                && name_chars.iter().enumerate().all(|(index, character)| {
                    string_ref(context, package_name, index) == Ok(*character)
                });
            if matches {
                return Ok(Some(package.as_word()));
            }
        }
        drop(packages);
        Ok(None)
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
        crate::with_root(ctx, &mut class, |_, class| {
            self.classes
                .lock()
                .map_err(|_| ObjectError::Storage(StorageCondition::ThreadNotRegistered))?
                .insert(name, crate::runtime::RootedWord::new(&self.heap, *class));
            Ok(())
        })
    }

    #[must_use]
    pub fn class(&self, _ctx: &mut ThreadContext, name: &str) -> Option<Word> {
        self.classes
            .lock()
            .ok()?
            .get(name)
            .map(crate::runtime::RootedWord::get)
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

use crate::hash_table::HashTable;
use crate::{ObjectError, Package, Runtime, ThreadContext, make_string, string_length, string_ref};
use ncl_sys::{HeapConfig, Word};

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
        self.check_package_name(ctx, package, name)?;
        HashTable::from_word(Self::table(&self.packages)?).insert(ctx, self, name, package)
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
        Ok(HashTable::from_word(Self::table(&self.packages)?)
            .remove(ctx, self, name)?
            .is_some())
    }

    fn resolve_package(
        &self,
        context: &ThreadContext,
        name: &str,
    ) -> Result<Option<Word>, ObjectError> {
        let table = Self::table(&self.packages)?;
        let name_chars = name.chars().collect::<Vec<_>>();
        let mut result = None;
        let mut failure = None;
        HashTable::from_word(table).for_each_entry(context, |_, package| {
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

    fn resolve_package_name(
        &self,
        context: &ThreadContext,
        name: &str,
    ) -> Result<Option<Word>, ObjectError> {
        let table = Self::table(&self.packages)?;
        let name_chars = name.chars().collect::<Vec<_>>();
        let mut result = None;
        let mut failure = None;
        HashTable::from_word(table).for_each_entry(context, |_, package| {
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
            let package_name = match package.name(context) {
                Ok(package_name) => package_name,
                Err(error) => {
                    failure = Some(error);
                    return;
                }
            };
            let matches = string_length(context, package_name).ok() == Some(name_chars.len())
                && name_chars.iter().enumerate().all(|(index, character)| {
                    string_ref(context, package_name, index) == Ok(*character)
                });
            if matches {
                result = Some(package.as_word());
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

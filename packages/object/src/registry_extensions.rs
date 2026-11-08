use crate::object_access::put;
use crate::package::{NAME, NICKNAMES};
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

    fn check_package_name(
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
        Ok(())
    }

    fn register_package_name(
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
                    Package::from_word(*package).add_nickname(ctx, self, *name)?;
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

    fn unregister_package_name(
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

    /// Rename a package and replace its registered nicknames.
    ///
    /// # Errors
    /// Returns an object error when the package is locked, a name is
    /// malformed, or a name or nickname conflicts with another package.
    pub fn rename_package(
        &self,
        ctx: &mut ThreadContext,
        package: Package,
        name: Word,
        nicknames: Word,
    ) -> Result<(), ObjectError> {
        let mut package_word = package.as_word();
        crate::with_root(ctx, &mut package_word, |ctx, package| {
            Package::from_word(*package).ensure_unlocked(ctx)?;
            let mut name = name;
            crate::with_root(ctx, &mut name, |ctx, name| {
                string_length(ctx, *name)?;
                let mut nicknames = nicknames;
                crate::with_root(ctx, &mut nicknames, |ctx, nicknames| {
                    let mut names = *nicknames;
                    while names != Word::NIL {
                        let nickname = ncl_sys::read_cons_word(&ctx.thread, names, 0)
                            .ok_or(ObjectError::Layout)?;
                        string_length(ctx, nickname)?;
                        self.check_package_name(ctx, *package, nickname)?;
                        names = ncl_sys::read_cons_word(&ctx.thread, names, 1)
                            .ok_or(ObjectError::Layout)?;
                    }
                    self.check_package_name(ctx, *package, *name)?;

                    let old_name = Package::from_word(*package).name(ctx)?;
                    self.unregister_package_name(ctx, old_name)?;
                    let old_nicknames = Package::from_word(*package).nicknames(ctx)?;
                    let mut old = old_nicknames;
                    while old != Word::NIL {
                        let nickname = ncl_sys::read_cons_word(&ctx.thread, old, 0)
                            .ok_or(ObjectError::Layout)?;
                        self.unregister_package_name(ctx, nickname)?;
                        old = ncl_sys::read_cons_word(&ctx.thread, old, 1)
                            .ok_or(ObjectError::Layout)?;
                    }
                    put(ctx, *package, NAME, *name)?;
                    put(ctx, *package, NICKNAMES, *nicknames)?;
                    self.register_package_name(ctx, *package, *name, false)?;
                    let mut names = *nicknames;
                    while names != Word::NIL {
                        let nickname = ncl_sys::read_cons_word(&ctx.thread, names, 0)
                            .ok_or(ObjectError::Layout)?;
                        self.register_package_name(ctx, *package, nickname, true)?;
                        names = ncl_sys::read_cons_word(&ctx.thread, names, 1)
                            .ok_or(ObjectError::Layout)?;
                    }
                    Ok(())
                })
            })
        })
    }

    /// Remove a package from the runtime package registry.
    ///
    /// # Errors
    /// Returns an object error when the package is locked, its metadata is
    /// malformed, or the registry cannot be updated.
    pub fn delete_package(
        &self,
        ctx: &mut ThreadContext,
        package: Package,
    ) -> Result<bool, ObjectError> {
        let mut package_word = package.as_word();
        crate::with_root(ctx, &mut package_word, |ctx, package| {
            Package::from_word(*package).ensure_unlocked(ctx)?;
            let mut name = Package::from_word(*package).name(ctx)?;
            crate::with_root(ctx, &mut name, |ctx, name| {
                let mut nicknames = Package::from_word(*package).nicknames(ctx)?;
                let mut removed = self.unregister_package_name(ctx, *name)?;
                while nicknames != Word::NIL {
                    let next = crate::with_root(ctx, &mut nicknames, |ctx, nicknames_slot| {
                        let mut nickname = ncl_sys::read_cons_word(&ctx.thread, *nicknames_slot, 0)
                            .ok_or(ObjectError::Layout)?;
                        crate::with_root(ctx, &mut nickname, |ctx, nickname| {
                            removed |= self.unregister_package_name(ctx, *nickname)?;
                            Ok(())
                        })?;
                        ncl_sys::read_cons_word(&ctx.thread, *nicknames_slot, 1)
                            .ok_or(ObjectError::Layout)
                    })?;
                    nicknames = next;
                }
                if removed {
                    put(ctx, *package, NAME, Word::NIL)?;
                }
                Ok(removed)
            })
        })
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

#[cfg(test)]
mod registry_behavior_tests {
    use super::*;
    use crate::{make_cons, make_string};

    #[test]
    fn package_lifecycle_and_class_registry_preserve_names() -> Result<(), ObjectError> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;

        let common_lisp = runtime.ensure_package(&mut ctx, "CL")?;
        assert_eq!(
            runtime.ensure_package(&mut ctx, "COMMON-LISP")?,
            common_lisp
        );
        assert_eq!(runtime.find_package(&ctx, "CL"), Some(common_lisp));

        let package = runtime.ensure_package(&mut ctx, "REGISTRY-LIFECYCLE")?;
        let nickname = make_string(&mut ctx, &runtime, &['R', 'L'])?;
        let nicknames = make_cons(&mut ctx, &runtime, nickname, Word::NIL)?;
        let renamed = make_string(&mut ctx, &runtime, &['R', 'E', 'N', 'A', 'M', 'E', 'D'])?;
        runtime.rename_package(&mut ctx, Package::from_word(package), renamed, nicknames)?;
        assert_eq!(runtime.find_package(&ctx, "REGISTRY-LIFECYCLE"), None);
        assert_eq!(runtime.find_package(&ctx, "RENAMED"), Some(package));
        assert_eq!(runtime.find_package(&ctx, "RL"), Some(package));

        runtime.define_class(&mut ctx, "REGISTRY-CLASS", Word::TRUE)?;
        assert_eq!(runtime.class(&mut ctx, "REGISTRY-CLASS"), Some(Word::TRUE));
        assert_eq!(runtime.class(&mut ctx, "UNKNOWN-CLASS"), None);
        runtime.add_feature("REGISTRY-FEATURE");
        runtime.add_feature("REGISTRY-FEATURE");
        assert_eq!(runtime.features(), vec!["REGISTRY-FEATURE"]);

        assert!(runtime.all_packages(&ctx)?.contains(&package));
        assert!(runtime.delete_package(&mut ctx, Package::from_word(package))?);
        assert_eq!(runtime.find_package(&ctx, "RENAMED"), None);
        assert_eq!(runtime.find_package(&ctx, "RL"), None);
        assert!(!runtime.all_packages(&ctx)?.contains(&package));
        assert!(!runtime.delete_package(&mut ctx, Package::from_word(package))?);
        Ok(())
    }

    #[test]
    fn common_lisp_user_has_standard_cl_user_nickname() -> Result<(), ObjectError> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        let common_lisp_user = runtime
            .find_package(&ctx, "COMMON-LISP-USER")
            .ok_or(ObjectError::Layout)?;

        assert_eq!(
            runtime.find_package(&ctx, "CL-USER"),
            Some(common_lisp_user)
        );
        Ok(())
    }
}

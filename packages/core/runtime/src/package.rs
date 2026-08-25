use std::collections::{HashMap, HashSet};

#[path = "package_definition.rs"]
mod package_definition;

pub(crate) const DEFAULT_PACKAGE: &str = "NCL-USER";
pub(crate) const COMMON_LISP_PACKAGE: &str = "COMMON-LISP";
pub(crate) const KEYWORD_PACKAGE: &str = "KEYWORD";

#[derive(Clone, Debug)]
struct Package {
    use_packages: Vec<String>,
    exports: HashSet<String>,
    symbols: HashSet<String>,
    imports: HashMap<String, String>,
    shadows: HashSet<String>,
    documentation: Option<String>,
    local_nicknames: HashMap<String, String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SymbolStatus {
    Internal,
    External,
    Inherited,
}

#[derive(Clone, Debug)]
pub(crate) struct PackageState {
    current: String,
    packages: HashMap<String, Package>,
    nicknames: HashMap<String, String>,
    renamed_packages: HashMap<String, String>,
}

impl PackageState {
    pub(crate) fn new() -> Self {
        let mut packages = HashMap::new();
        packages.insert(
            COMMON_LISP_PACKAGE.to_string(),
            Package {
                use_packages: Vec::new(),
                exports: HashSet::new(),
                symbols: HashSet::new(),
                imports: HashMap::new(),
                shadows: HashSet::new(),
                documentation: None,
                local_nicknames: HashMap::new(),
            },
        );
        packages.insert(
            DEFAULT_PACKAGE.to_string(),
            Package {
                use_packages: vec![COMMON_LISP_PACKAGE.to_string()],
                exports: HashSet::new(),
                symbols: HashSet::new(),
                imports: HashMap::new(),
                shadows: HashSet::new(),
                documentation: None,
                local_nicknames: HashMap::new(),
            },
        );
        packages.insert(
            KEYWORD_PACKAGE.to_string(),
            Package {
                use_packages: Vec::new(),
                exports: HashSet::new(),
                symbols: HashSet::new(),
                imports: HashMap::new(),
                shadows: HashSet::new(),
                documentation: None,
                local_nicknames: HashMap::new(),
            },
        );
        Self {
            current: DEFAULT_PACKAGE.to_string(),
            packages,
            nicknames: HashMap::new(),
            renamed_packages: HashMap::new(),
        }
    }

    pub(crate) fn current(&self) -> &str {
        &self.current
    }

    pub(crate) fn package_object_name(&self, name: &str) -> String {
        let input = canonical_package_name_input(name);
        let mut current = input.clone();
        let mut seen = HashSet::new();
        while let Some(next) = self.renamed_packages.get(&current) {
            if !seen.insert(current.clone()) {
                break;
            }
            current = next.clone();
        }
        if self.packages.contains_key(&current) {
            current
        } else {
            self.canonical_package_name(&current)
        }
    }

    pub(crate) fn canonical_package_name(&self, name: &str) -> String {
        let name = canonical_package_name_input(name);
        self.nicknames.get(&name).cloned().unwrap_or(name)
    }

    pub(crate) fn canonical_package_name_for(&self, current: &str, name: &str) -> String {
        let current = self.canonical_package_name(current);
        let name = canonical_package_name_input(name);
        let local_target = self
            .packages
            .get(&current)
            .and_then(|package| package.local_nicknames.get(&name))
            .cloned();
        local_target.unwrap_or_else(|| self.canonical_package_name(&name))
    }

    pub(crate) fn package_exists(&self, name: &str) -> bool {
        self.packages
            .contains_key(&self.canonical_package_name(name))
    }

    pub(crate) fn package_documentation(&self, package: &str) -> Option<String> {
        let package = self.canonical_package_name(package);
        self.packages
            .get(&package)
            .and_then(|entry| entry.documentation.clone())
    }

    pub(crate) fn is_exported(&self, package: &str, name: &str) -> bool {
        let package = self.canonical_package_name(package);
        let name = normalize_symbol_name(name);
        if package == COMMON_LISP_PACKAGE || package == KEYWORD_PACKAGE {
            return true;
        }
        self.packages
            .get(&package)
            .is_some_and(|entry| entry.exports.contains(&name))
    }

    fn imported_reference(&self, reference: &str, exact: bool) -> SymbolReference {
        let (source_package, source_name) = split_symbol(reference)
            .map(|(package, name, _)| (package.to_string(), name.to_string()))
            .unwrap_or_else(|| (DEFAULT_PACKAGE.to_string(), reference.to_string()));
        let source_package = self.canonical_package_name(&source_package);
        let source_name = if exact {
            source_name
        } else {
            normalize_symbol_name(&source_name)
        };
        SymbolReference::new(source_package, source_name)
    }

    fn local_symbol_reference(
        &self,
        package: &str,
        name: &str,
        imported: Option<String>,
        exact: bool,
    ) -> SymbolReference {
        imported
            .map(|reference| self.imported_reference(&reference, exact))
            .unwrap_or_else(|| SymbolReference::new(package.to_string(), name.to_string()))
    }

    fn exported_symbol_reference(
        &self,
        package: &str,
        name: &str,
        exact: bool,
    ) -> Option<SymbolReference> {
        let imported = self.packages.get(package).and_then(|entry| {
            if entry.symbols.contains(name) && entry.exports.contains(name) {
                entry.imports.get(name).cloned()
            } else {
                None
            }
        });
        let exists = self
            .packages
            .get(package)
            .is_some_and(|entry| entry.symbols.contains(name) && entry.exports.contains(name));
        exists.then(|| self.local_symbol_reference(package, name, imported, exact))
    }

    fn find_symbol_inner(
        &self,
        package: &str,
        name: &str,
        exact: bool,
    ) -> Option<(SymbolReference, SymbolStatus)> {
        let package = self.canonical_package_name(package);
        let name = if exact {
            name.to_string()
        } else {
            normalize_symbol_name(name)
        };
        let (local_import, local, shadowed, use_packages) = self
            .packages
            .get(&package)
            .map(|entry| {
                (
                    entry.imports.get(&name).cloned(),
                    entry.symbols.contains(&name),
                    entry.shadows.contains(&name),
                    entry.use_packages.clone(),
                )
            })
            .unwrap_or_default();
        if local {
            let exported = self
                .packages
                .get(&package)
                .is_some_and(|entry| entry.exports.contains(&name));
            let status = if package == KEYWORD_PACKAGE || exported {
                SymbolStatus::External
            } else {
                SymbolStatus::Internal
            };
            return Some((
                self.local_symbol_reference(&package, &name, local_import, exact),
                status,
            ));
        }
        if shadowed {
            return None;
        }
        for used_package in use_packages {
            let used_package = self.canonical_package_name(&used_package);
            if let Some(reference) = self.exported_symbol_reference(&used_package, &name, exact) {
                return Some((reference, SymbolStatus::Inherited));
            }
        }
        None
    }

    pub(crate) fn find_symbol(
        &self,
        package: &str,
        name: &str,
    ) -> Option<(SymbolReference, SymbolStatus)> {
        self.find_symbol_inner(package, name, false)
    }

    pub(crate) fn find_symbol_exact(
        &self,
        package: &str,
        name: &str,
    ) -> Option<(SymbolReference, SymbolStatus)> {
        self.find_symbol_inner(package, name, true)
    }

    pub(crate) fn find_all_symbols(&self, name: &str) -> Vec<SymbolReference> {
        let mut symbols = Vec::new();
        for (package_name, package) in &self.packages {
            if !package.symbols.contains(name) {
                continue;
            }
            if let Some((reference, _)) = self.find_symbol_exact(package_name, name)
                && !symbols.contains(&reference)
            {
                symbols.push(reference);
            }
        }
        symbols.sort_by(|left, right| {
            left.package()
                .cmp(right.package())
                .then_with(|| left.name().cmp(right.name()))
        });
        symbols
    }

    pub(crate) fn symbol_status(&self, package: &str, name: &str) -> Option<SymbolStatus> {
        self.find_symbol(package, name).map(|(_, status)| status)
    }

    pub(crate) fn symbol_status_exact(&self, package: &str, name: &str) -> Option<SymbolStatus> {
        self.find_symbol_exact(package, name)
            .map(|(_, status)| status)
    }

    pub(crate) fn intern_symbol(&mut self, package: &str, name: &str) -> Option<SymbolStatus> {
        let package = self.canonical_package_name(package);
        let name = normalize_symbol_name(name);
        if let Some((_, status)) = self.find_symbol(&package, &name) {
            return Some(status);
        }
        self.packages.get_mut(&package).map(|entry| {
            entry.symbols.insert(name.clone());
            if package == KEYWORD_PACKAGE || entry.exports.contains(&name) {
                SymbolStatus::External
            } else {
                SymbolStatus::Internal
            }
        })
    }

    pub(crate) fn intern_symbol_exact(
        &mut self,
        package: &str,
        name: &str,
    ) -> Option<(SymbolStatus, bool)> {
        let package = self.canonical_package_name(package);
        if let Some((_, status)) = self.find_symbol_exact(&package, name) {
            return Some((status, false));
        }
        self.packages.get_mut(&package).map(|entry| {
            let inserted = entry.symbols.insert(name.to_string());
            let status = if package == KEYWORD_PACKAGE || entry.exports.contains(name) {
                SymbolStatus::External
            } else {
                SymbolStatus::Internal
            };
            (status, inserted)
        })
    }

    pub(crate) fn ensure_symbol(&mut self, package: &str, name: &str) {
        let package = self.canonical_package_name(package);
        let name = normalize_symbol_name(name);
        if let Some(entry) = self.packages.get_mut(&package) {
            entry.symbols.insert(name);
        }
    }

    pub(crate) fn use_packages_for(&self, name: &str) -> Vec<String> {
        let name = self.canonical_package_name(name);
        self.packages
            .get(&name)
            .map(|package| package.use_packages.clone())
            .unwrap_or_default()
    }

    pub(crate) fn nicknames_for(&self, name: &str) -> Vec<String> {
        let name = self.canonical_package_name(name);
        let mut nicknames = self
            .nicknames
            .iter()
            .filter_map(|(nickname, package)| (package == &name).then(|| nickname.clone()))
            .collect::<Vec<_>>();
        nicknames.sort();
        nicknames
    }

    pub(crate) fn shadowing_symbols_for(&self, name: &str) -> Vec<String> {
        let name = self.canonical_package_name(name);
        let mut symbols = self
            .packages
            .get(&name)
            .map(|package| package.shadows.iter().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        symbols.sort();
        symbols
    }

    pub(crate) fn used_by_packages_for(&self, name: &str) -> Vec<String> {
        let name = self.canonical_package_name(name);
        let mut packages = self
            .packages
            .iter()
            .filter_map(|(package_name, package)| {
                package
                    .use_packages
                    .iter()
                    .any(|used_package| used_package == &name)
                    .then(|| package_name.clone())
            })
            .collect::<Vec<_>>();
        packages.sort();
        packages
    }

    pub(crate) fn delete_package(&mut self, name: &str, current: &str) -> Result<(), String> {
        let name = self.package_object_name(name);
        let current = self.package_object_name(current);
        if !self.packages.contains_key(&name) {
            return Err(format!("unknown package {name}"));
        }
        if name == current {
            return Err("*PACKAGE* can't be a deleted package".to_string());
        }
        let used_by = self.used_by_packages_for(&name);
        if !used_by.is_empty() {
            let users = used_by
                .iter()
                .map(|package| format!("\"{package}\""))
                .collect::<Vec<_>>()
                .join(" ");
            return Err(format!(
                "Package \"{name}\" is used by package:({users})"
            ));
        }

        self.packages.remove(&name);
        self.nicknames.retain(|_, package| package != &name);
        for package in self.packages.values_mut() {
            package
                .local_nicknames
                .retain(|_, target| target != &name);
        }
        let aliases = self.renamed_packages.keys().cloned().collect::<Vec<_>>();
        for alias in aliases {
            if self.package_object_name(&alias) == name {
                self.renamed_packages.remove(&alias);
            }
        }
        self.renamed_packages.remove(&name);
        Ok(())
    }

    pub(crate) fn rename_package(
        &mut self,
        name: &str,
        new_name: &str,
        new_nicknames: Vec<String>,
    ) -> Result<String, String> {
        let old_name = self.package_object_name(name);
        if !self.packages.contains_key(&old_name) {
            return Err(format!("unknown package {old_name}"));
        }

        let new_name = canonical_package_name_input(new_name);
        if new_name.is_empty() {
            return Err("package name cannot be empty".to_string());
        }
        let existing_name = self.package_object_name(&new_name);
        if (self.packages.contains_key(&new_name) && new_name != old_name)
            || (self.packages.contains_key(&existing_name) && existing_name != old_name)
            || self
                .nicknames
                .get(&new_name)
                .is_some_and(|owner| owner != &old_name)
        {
            return Err(format!("package {new_name} already exists"));
        }

        let mut nicknames = Vec::new();
        for nickname in new_nicknames {
            let nickname = canonical_package_name_input(&nickname);
            if nickname.is_empty() || nickname == new_name {
                return Err(format!("invalid package nickname {nickname}"));
            }
            let existing_name = self.package_object_name(&nickname);
            if (self.packages.contains_key(&nickname) && nickname != old_name)
                || (self.packages.contains_key(&existing_name) && existing_name != old_name)
                || self
                    .nicknames
                    .get(&nickname)
                    .is_some_and(|owner| owner != &old_name)
            {
                return Err(format!("package nickname {nickname} is already in use"));
            }
            if !nicknames.contains(&nickname) {
                nicknames.push(nickname);
            }
        }

        let mut package = self
            .packages
            .remove(&old_name)
            .expect("package existence was checked before rename");
        rename_package_references(&mut package, &old_name, &new_name);
        for package in self.packages.values_mut() {
            rename_package_references(package, &old_name, &new_name);
        }

        if self.current == old_name {
            self.current = new_name.clone();
        }
        self.nicknames.retain(|_, owner| owner != &old_name);
        self.nicknames.remove(&new_name);
        for nickname in &nicknames {
            self.nicknames.insert(nickname.clone(), new_name.clone());
        }
        for target in self.renamed_packages.values_mut() {
            if target == &old_name {
                *target = new_name.clone();
            }
        }
        self.renamed_packages.remove(&new_name);
        if old_name == new_name {
            self.renamed_packages.remove(&old_name);
        } else {
            self.renamed_packages
                .insert(old_name.clone(), new_name.clone());
        }
        self.packages.insert(new_name.clone(), package);
        Ok(new_name)
    }

    pub(crate) fn use_package(&mut self, package: &str, target: &str) {
        let package = self.canonical_package_name(package);
        let target = self.canonical_package_name(target);
        if let Some(entry) = self.packages.get_mut(&target)
            && !entry.use_packages.iter().any(|used| used == &package)
        {
            entry.use_packages.push(package);
        }
    }

    fn use_package_conflict_inner(
        &self,
        package: &str,
        target: &str,
        exact: bool,
    ) -> Option<String> {
        let package = self.canonical_package_name(package);
        let target = self.canonical_package_name(target);
        let names = self
            .packages
            .get(&package)
            .map(|entry| entry.exports.iter().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        for name in names {
            let Some(source) = self.exported_symbol_reference(&package, &name, exact) else {
                continue;
            };
            let existing = if exact {
                self.find_symbol_exact(&target, &name)
            } else {
                self.find_symbol(&target, &name)
            };
            if existing.is_some_and(|(reference, _)| reference != source) {
                return Some(name);
            }
        }
        None
    }

    pub(crate) fn use_package_conflict(&self, package: &str, target: &str) -> Option<String> {
        self.use_package_conflict_inner(package, target, false)
    }

    pub(crate) fn use_package_conflict_exact(&self, package: &str, target: &str) -> Option<String> {
        self.use_package_conflict_inner(package, target, true)
    }

    pub(crate) fn unuse_package(&mut self, package: &str, target: &str) {
        let package = self.canonical_package_name(package);
        let target = self.canonical_package_name(target);
        if let Some(entry) = self.packages.get_mut(&target) {
            entry.use_packages.retain(|used| used != &package);
        }
    }

    pub(crate) fn imported_symbol_for(&self, package: &str, name: &str) -> Option<String> {
        let package = self.canonical_package_name(package);
        let name = normalize_symbol_name(name);
        let reference = self
            .packages
            .get(&package)
            .and_then(|entry| entry.imports.get(&name).cloned())?;
        let reference = self.imported_reference(&reference, false);
        Some(canonical_symbol_name(reference.package(), reference.name()))
    }

    pub(crate) fn is_shadowed(&self, package: &str, name: &str) -> bool {
        let package = self.canonical_package_name(package);
        let name = normalize_symbol_name(name);
        self.packages
            .get(&package)
            .is_some_and(|entry| entry.shadows.contains(&name))
    }

    pub(crate) fn symbol_exists(&self, package: &str, name: &str) -> bool {
        let package = self.canonical_package_name(package);
        package == COMMON_LISP_PACKAGE
            || package == KEYWORD_PACKAGE
            || self.symbol_status(&package, name).is_some()
    }

    pub(crate) fn symbol_exists_exact(&self, package: &str, name: &str) -> bool {
        let package = self.canonical_package_name(package);
        package == COMMON_LISP_PACKAGE
            || package == KEYWORD_PACKAGE
            || self.symbol_status_exact(&package, name).is_some()
    }

    fn source_symbol_reference(
        &self,
        package: &str,
        name: &str,
        exact: bool,
    ) -> Option<SymbolReference> {
        let package = self.canonical_package_name(package);
        let result = if exact {
            self.find_symbol_exact(&package, name)
        } else {
            self.find_symbol(&package, name)
        };
        result.map(|(reference, _)| reference).or_else(|| {
            (package == COMMON_LISP_PACKAGE || package == KEYWORD_PACKAGE).then(|| {
                SymbolReference::new(
                    package,
                    if exact {
                        name.to_string()
                    } else {
                        normalize_symbol_name(name)
                    },
                )
            })
        })
    }

    fn import_conflict_inner(
        &self,
        source_package: &str,
        source_name: &str,
        target: &str,
        exact: bool,
    ) -> bool {
        let Some(source) = self.source_symbol_reference(source_package, source_name, exact) else {
            return false;
        };
        let target_name = if exact {
            source_name.to_string()
        } else {
            normalize_symbol_name(source_name)
        };
        let existing = if exact {
            self.find_symbol_exact(target, &target_name)
        } else {
            self.find_symbol(target, &target_name)
        };
        existing.is_some_and(|(reference, _)| reference != source)
    }

    pub(crate) fn import_conflict(
        &self,
        source_package: &str,
        source_name: &str,
        target: &str,
    ) -> bool {
        self.import_conflict_inner(source_package, source_name, target, false)
    }

    pub(crate) fn import_conflict_exact(
        &self,
        source_package: &str,
        source_name: &str,
        target: &str,
    ) -> bool {
        self.import_conflict_inner(source_package, source_name, target, true)
    }

    pub(crate) fn imported_symbol_name(&self, package: &str, name: &str) -> String {
        self.imported_symbol_for(package, name)
            .unwrap_or_else(|| canonical_symbol_name(package, name))
    }

    pub(crate) fn imported_symbol_parts_exact(
        &self,
        package: &str,
        name: &str,
    ) -> (String, String) {
        let package = self.canonical_package_name(package);
        self.packages
            .get(&package)
            .and_then(|entry| entry.imports.get(name))
            .and_then(|reference| split_symbol(reference))
            .map(|(source_package, source_name, _)| {
                (
                    self.canonical_package_name(source_package),
                    source_name.to_string(),
                )
            })
            .unwrap_or((package, name.to_string()))
    }

    pub(crate) fn import_symbol(
        &mut self,
        source_package: &str,
        source_name: &str,
        target: &str,
        shadowing: bool,
    ) {
        let source_package = self.canonical_package_name(source_package);
        let source_name = normalize_symbol_name(source_name);
        let target = self.canonical_package_name(target);
        if let Some(entry) = self.packages.get_mut(&target) {
            entry.symbols.insert(source_name.clone());
            entry.imports.insert(
                source_name.clone(),
                format!("{source_package}::{source_name}"),
            );
            if shadowing {
                entry.shadows.insert(source_name);
            } else {
                entry.shadows.remove(&source_name);
            }
        }
    }

    pub(crate) fn import_symbol_exact(
        &mut self,
        source_package: &str,
        source_name: &str,
        target: &str,
        shadowing: bool,
    ) {
        let source_package = self.canonical_package_name(source_package);
        let source_name = source_name.to_string();
        let target = self.canonical_package_name(target);
        if let Some(entry) = self.packages.get_mut(&target) {
            entry.symbols.insert(source_name.clone());
            entry.imports.insert(
                source_name.clone(),
                format!("{source_package}::{source_name}"),
            );
            if shadowing {
                entry.shadows.insert(source_name);
            } else {
                entry.shadows.remove(&source_name);
            }
        }
    }

    pub(crate) fn shadow_symbol(&mut self, package: &str, name: &str) {
        let package = self.canonical_package_name(package);
        let name = normalize_symbol_name(name);
        if let Some(entry) = self.packages.get_mut(&package) {
            entry.symbols.insert(name.clone());
            entry.imports.remove(&name);
            entry.shadows.insert(name);
        }
    }

    pub(crate) fn shadow_symbol_exact(&mut self, package: &str, name: &str) {
        let package = self.canonical_package_name(package);
        let name = name.to_string();
        if let Some(entry) = self.packages.get_mut(&package) {
            entry.symbols.insert(name.clone());
            entry.imports.remove(&name);
            entry.shadows.insert(name);
        }
    }

    pub(crate) fn unintern_symbol(&mut self, package: &str, name: &str) -> bool {
        let package = self.canonical_package_name(package);
        let name = normalize_symbol_name(name);
        self.packages.get_mut(&package).is_some_and(|entry| {
            let mut existed = entry.symbols.remove(&name);
            existed |= entry.exports.remove(&name);
            existed |= entry.imports.remove(&name).is_some();
            existed |= entry.shadows.remove(&name);
            existed
        })
    }

    pub(crate) fn unintern_symbol_exact(&mut self, package: &str, name: &str) -> bool {
        let package = self.canonical_package_name(package);
        self.packages.get_mut(&package).is_some_and(|entry| {
            let mut existed = entry.symbols.remove(name);
            existed |= entry.exports.remove(name);
            existed |= entry.imports.remove(name).is_some();
            existed |= entry.shadows.remove(name);
            existed
        })
    }

    pub(crate) fn unintern_symbol_reference(
        &mut self,
        package: &str,
        source_package: &str,
        source_name: &str,
        exact: bool,
    ) -> bool {
        let package = self.canonical_package_name(package);
        let source_package = self.canonical_package_name(source_package);
        let name = if exact {
            source_name.to_string()
        } else {
            normalize_symbol_name(source_name)
        };
        let matches = self.packages.get(&package).is_some_and(|entry| {
            if let Some(reference) = entry.imports.get(&name) {
                let Some((reference_package, reference_name, _)) = split_symbol(reference) else {
                    return false;
                };
                let reference_package = self.canonical_package_name(reference_package);
                let reference_name = if exact {
                    reference_name.to_string()
                } else {
                    normalize_symbol_name(reference_name)
                };
                reference_package == source_package && reference_name == name
            } else {
                package == source_package && entry.symbols.contains(&name)
            }
        });
        if !matches {
            return false;
        }
        self.packages.get_mut(&package).is_some_and(|entry| {
            let mut existed = entry.symbols.remove(&name);
            existed |= entry.exports.remove(&name);
            existed |= entry.imports.remove(&name).is_some();
            existed |= entry.shadows.remove(&name);
            existed
        })
    }

    pub(crate) fn export_symbols(&mut self, package: &str, symbols: &[String]) {
        let package = self.canonical_package_name(package);
        if let Some(entry) = self.packages.get_mut(&package) {
            for symbol in symbols {
                let symbol = normalize_symbol_name(symbol);
                entry.symbols.insert(symbol.clone());
                entry.exports.insert(symbol);
            }
        }
    }

    pub(crate) fn export_symbols_exact(&mut self, package: &str, symbols: &[String]) {
        let package = self.canonical_package_name(package);
        if let Some(entry) = self.packages.get_mut(&package) {
            for symbol in symbols {
                entry.symbols.insert(symbol.clone());
                entry.exports.insert(symbol.clone());
            }
        }
    }

    pub(crate) fn unexport_symbols(&mut self, package: &str, symbols: &[String]) {
        let package = self.canonical_package_name(package);
        if let Some(entry) = self.packages.get_mut(&package) {
            for symbol in symbols {
                entry.exports.remove(&normalize_symbol_name(symbol));
            }
        }
    }

    pub(crate) fn unexport_symbols_exact(&mut self, package: &str, symbols: &[String]) {
        let package = self.canonical_package_name(package);
        if let Some(entry) = self.packages.get_mut(&package) {
            for symbol in symbols {
                entry.exports.remove(symbol);
            }
        }
    }

    pub(crate) fn all_package_names(&self) -> Vec<String> {
        let mut names = self.packages.keys().cloned().collect::<Vec<_>>();
        names.sort();
        names
    }

    pub(crate) fn define_package(
        &mut self,
        name: String,
        nicknames: Vec<String>,
        use_packages: Vec<String>,
        exports: HashSet<String>,
        documentation: Option<String>,
        local_nicknames: HashMap<String, String>,
    ) -> Result<(), String> {
        let package_definition::PreparedPackage {
            name,
            nicknames,
            use_packages,
            exports,
            documentation,
            local_nicknames,
        } = package_definition::prepare(
            self,
            name,
            nicknames,
            use_packages,
            exports,
            documentation,
            local_nicknames,
        )?;

        for (left_index, left_package) in use_packages.iter().enumerate() {
            let left_exports = self
                .packages
                .get(left_package)
                .map(|entry| entry.exports.clone())
                .unwrap_or_default();
            for right_package in use_packages.iter().skip(left_index + 1) {
                if left_package == right_package {
                    continue;
                }
                let right_exports = self
                    .packages
                    .get(right_package)
                    .map(|entry| entry.exports.clone())
                    .unwrap_or_default();
                for symbol in left_exports.intersection(&right_exports) {
                    let left_reference = self
                        .exported_symbol_reference(left_package, symbol, false)
                        .expect("exported symbol must have a reference");
                    let right_reference = self
                        .exported_symbol_reference(right_package, symbol, false)
                        .expect("exported symbol must have a reference");
                    if left_reference != right_reference {
                        return Err(format!(
                            "name conflict for symbol {symbol} between packages {left_package} and {right_package}"
                        ));
                    }
                }
            }
        }

        self.renamed_packages.remove(&name);
        self.nicknames.retain(|_, package| package != &name);
        self.packages.insert(
            name.clone(),
            Package {
                use_packages,
                symbols: exports.clone(),
                exports,
                imports: HashMap::new(),
                shadows: HashSet::new(),
                documentation,
                local_nicknames,
            },
        );
        for nickname in nicknames {
            self.nicknames.insert(nickname, name.clone());
        }
        Ok(())
    }

    pub(crate) fn set_current(&mut self, name: String) {
        self.current = self.canonical_package_name(&name);
    }
}

fn rename_package_references(package: &mut Package, old_name: &str, new_name: &str) {
    for used_package in &mut package.use_packages {
        if used_package == old_name {
            *used_package = new_name.to_string();
        }
    }
    for target in package.local_nicknames.values_mut() {
        if target == old_name {
            *target = new_name.to_string();
        }
    }
    for reference in package.imports.values_mut() {
        let replacement = split_symbol(reference).and_then(|(source, symbol, _)| {
            (canonical_package_name_input(source) == old_name)
                .then(|| format!("{new_name}::{symbol}"))
        });
        if let Some(replacement) = replacement {
            *reference = replacement;
        }
    }
}

pub(crate) fn normalize_package_name(name: &str) -> String {
    let name = name.strip_prefix(':').unwrap_or(name);
    let name = name.to_ascii_uppercase();
    if name == "CL" {
        COMMON_LISP_PACKAGE.to_string()
    } else {
        name
    }
}

fn canonical_package_name_input(name: &str) -> String {
    let name = name.strip_prefix(':').unwrap_or(name);
    if name == "CL" {
        COMMON_LISP_PACKAGE.to_string()
    } else {
        name.to_string()
    }
}

pub(crate) fn normalize_symbol_name(name: &str) -> String {
    name.to_ascii_uppercase()
}

pub(crate) fn canonical_symbol_name(package: &str, name: &str) -> String {
    let package = canonical_package_name_input(package);
    let name = normalize_symbol_name(name);
    if package == DEFAULT_PACKAGE {
        name
    } else {
        format!("{package}::{name}")
    }
}

pub(crate) fn canonical_symbol_name_exact(package: &str, name: &str) -> String {
    let package = canonical_package_name_input(package);
    if package == DEFAULT_PACKAGE {
        name.to_string()
    } else {
        format!("{package}::{name}")
    }
}

pub(crate) fn split_symbol(name: &str) -> Option<(&str, &str, bool)> {
    if let Some((package, symbol)) = name.split_once("::") {
        return Some((package, symbol, false));
    }
    name.split_once(':')
        .map(|(package, symbol)| (package, symbol, true))
}

use super::names::{canonical_symbol_name, normalize_package_name, normalize_symbol_name};
use super::{KEYWORD_PACKAGE, PackageState, SymbolStatus};

impl PackageState {
    pub(crate) fn rename_package(
        &mut self,
        old_name: &str,
        new_name: &str,
        nicknames: Vec<String>,
    ) -> Result<bool, String> {
        let old_name = self.canonical_package_name(old_name);
        let new_name = normalize_package_name(new_name);
        if !self.packages.contains_key(&old_name) {
            return Ok(false);
        }
        if new_name.is_empty() {
            return Err("package name cannot be empty".to_string());
        }
        if self.packages.contains_key(&new_name) || self.nicknames.contains_key(&new_name) {
            return Err(format!("package name {new_name} conflicts with an existing package"));
        }
        let mut normalized_nicknames = Vec::new();
        for nickname in nicknames {
            let nickname = normalize_package_name(&nickname);
            if nickname.is_empty()
                || nickname == new_name
                || self.packages.contains_key(&nickname)
                || self.nicknames.contains_key(&nickname)
                || normalized_nicknames.contains(&nickname)
            {
                return Err(format!("invalid or conflicting package nickname {nickname}"));
            }
            normalized_nicknames.push(nickname);
        }
        let mut package = self.packages.remove(&old_name).expect("package existence checked");
        for used in &mut package.use_packages {
            if used == &old_name {
                *used = new_name.clone();
            }
        }
        self.nicknames.retain(|_, package| package != &old_name);
        for entry in self.packages.values_mut() {
            for used in &mut entry.use_packages {
                if used == &old_name {
                    *used = new_name.clone();
                }
            }
            for target in entry.local_nicknames.values_mut() {
                if target == &old_name {
                    *target = new_name.clone();
                }
            }
            for symbol in entry.imports.values_mut() {
                if symbol.starts_with(&format!("{old_name}::")) {
                    *symbol = symbol.replacen(&format!("{old_name}::"), &format!("{new_name}::"), 1);
                }
            }
        }
        if self.current == old_name {
            self.current = new_name.clone();
        }
        self.packages.insert(new_name.clone(), package);
        for nickname in normalized_nicknames {
            self.nicknames.insert(nickname, new_name.clone());
        }
        Ok(true)
    }

    pub(crate) fn delete_package(&mut self, name: &str) -> bool {
        let name = self.canonical_package_name(name);
        if name == super::COMMON_LISP_PACKAGE || name == super::KEYWORD_PACKAGE || name == self.current {
            return false;
        }
        let removed = self.packages.remove(&name).is_some();
        if removed {
            self.nicknames.retain(|_, package| package != &name);
            for package in self.packages.values_mut() {
                package.use_packages.retain(|used| used != &name);
                package.local_nicknames.retain(|_, target| target != &name);
            }
        }
        removed
    }

    pub(crate) fn intern_symbol(&mut self, package: &str, name: &str) -> Option<SymbolStatus> {
        let package = self.canonical_package_name(package);
        let name = normalize_symbol_name(name);
        self.packages.get_mut(&package).map(|entry| {
            entry.symbols.insert(name.clone());
            if package == KEYWORD_PACKAGE || entry.exports.contains(&name) {
                SymbolStatus::External
            } else {
                SymbolStatus::Internal
            }
        })
    }

    pub(crate) fn ensure_symbol(&mut self, package: &str, name: &str) {
        let package = self.canonical_package_name(package);
        let name = normalize_symbol_name(name);
        if let Some(entry) = self.packages.get_mut(&package) {
            entry.symbols.insert(name);
        }
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

    pub(crate) fn unuse_package(&mut self, package: &str, target: &str) {
        let package = self.canonical_package_name(package);
        let target = self.canonical_package_name(target);
        if let Some(entry) = self.packages.get_mut(&target) {
            entry.use_packages.retain(|used| used != &package);
        }
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
                canonical_symbol_name(&source_package, &source_name),
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

    pub(crate) fn unintern_symbol(&mut self, package: &str, name: &str) -> bool {
        let package = self.canonical_package_name(package);
        let name = normalize_symbol_name(name);
        self.packages.get_mut(&package).is_some_and(|entry| {
            // Every removal must run unconditionally: a name can be present
            // in more than one of these sets at once (e.g. exported AND
            // shadowed), and `||` would short-circuit after the first hit,
            // leaving the rest of the stale entries behind.
            let removed_from_symbols = entry.symbols.remove(&name);
            let removed_from_exports = entry.exports.remove(&name);
            let removed_from_imports = entry.imports.remove(&name).is_some();
            let removed_from_shadows = entry.shadows.remove(&name);
            removed_from_symbols
                || removed_from_exports
                || removed_from_imports
                || removed_from_shadows
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

    pub(crate) fn unexport_symbols(&mut self, package: &str, symbols: &[String]) {
        let package = self.canonical_package_name(package);
        if let Some(entry) = self.packages.get_mut(&package) {
            for symbol in symbols {
                entry.exports.remove(&normalize_symbol_name(symbol));
            }
        }
    }

    pub(crate) fn set_current(&mut self, name: &str) {
        self.current = self.canonical_package_name(name);
    }
}

use super::names::{canonical_symbol_name, normalize_package_name, normalize_symbol_name};
use super::{COMMON_LISP_PACKAGE, KEYWORD_PACKAGE, PackageState, SymbolStatus};

impl PackageState {
    pub(crate) fn current(&self) -> &str {
        &self.current
    }

    pub(crate) fn canonical_package_name(&self, name: &str) -> String {
        let name = normalize_package_name(name);
        self.nicknames.get(&name).cloned().unwrap_or(name)
    }

    pub(crate) fn canonical_package_name_for(&self, current: &str, name: &str) -> String {
        let current = self.canonical_package_name(current);
        let name = normalize_package_name(name);
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

    pub(crate) fn symbol_status(&self, package: &str, name: &str) -> Option<SymbolStatus> {
        let package = self.canonical_package_name(package);
        let name = normalize_symbol_name(name);
        self.packages.get(&package).and_then(|entry| {
            if !entry.symbols.contains(&name) {
                return None;
            }
            if package == KEYWORD_PACKAGE || entry.exports.contains(&name) {
                Some(SymbolStatus::External)
            } else {
                Some(SymbolStatus::Internal)
            }
        })
    }

    pub(crate) fn use_packages_for(&self, name: &str) -> Vec<String> {
        let name = self.canonical_package_name(name);
        self.packages
            .get(&name)
            .map(|package| package.use_packages.clone())
            .unwrap_or_default()
    }

    pub(crate) fn imported_symbol_for(&self, package: &str, name: &str) -> Option<String> {
        let package = self.canonical_package_name(package);
        let name = normalize_symbol_name(name);
        self.packages
            .get(&package)
            .and_then(|entry| entry.imports.get(&name).cloned())
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

    pub(crate) fn imported_symbol_name(&self, package: &str, name: &str) -> String {
        self.imported_symbol_for(package, name)
            .unwrap_or_else(|| canonical_symbol_name(package, name))
    }

    pub(crate) fn all_package_names(&self) -> Vec<String> {
        let mut names = self.packages.keys().cloned().collect::<Vec<_>>();
        names.sort();
        names
    }
}

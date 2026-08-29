use super::names::{canonical_symbol_name, normalize_symbol_name};
use super::{KEYWORD_PACKAGE, PackageState, SymbolStatus};

impl PackageState {
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

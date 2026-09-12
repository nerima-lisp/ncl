use super::names::{canonical_symbol_name, normalize_package_name, normalize_symbol_name};
use super::{COMMON_LISP_PACKAGE, KEYWORD_PACKAGE, PackageState, SymbolStatus};
use crate::value::{PackageObject, SymbolObject};

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

    pub(crate) fn package_object_for(&self, name: &str) -> Option<PackageObject> {
        let name = self.canonical_package_name(name);
        self.packages
            .get(&name)
            .map(|package| package.object.clone())
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

    pub(crate) fn symbol_status_exact(&self, package: &str, name: &str) -> Option<SymbolStatus> {
        let package = self.canonical_package_name(package);
        let exact_status = self.packages.get(&package).and_then(|entry| {
            entry.exact_symbols.contains(name).then(|| {
                if package == KEYWORD_PACKAGE || entry.exports.contains(name) {
                    SymbolStatus::External
                } else {
                    SymbolStatus::Internal
                }
            })
        });
        exact_status.or_else(|| self.symbol_status(&package, name))
    }

    pub(crate) fn use_packages_for(&self, name: &str) -> Vec<String> {
        let name = self.canonical_package_name(name);
        self.packages
            .get(&name)
            .map(|package| package.use_packages.clone())
            .unwrap_or_default()
    }

    pub(crate) fn package_used_by_list_for(&self, name: &str) -> Vec<String> {
        let name = self.canonical_package_name(name);
        let mut packages = self
            .packages
            .iter()
            .filter_map(|(package_name, package)| {
                package
                    .use_packages
                    .iter()
                    .any(|used| used == &name)
                    .then(|| package_name.clone())
            })
            .collect::<Vec<_>>();
        packages.sort();
        packages
    }

    pub(crate) fn package_nicknames_for(&self, name: &str) -> Vec<String> {
        let name = self.canonical_package_name(name);
        self.packages
            .get(&name)
            .map(|package| package.nicknames.clone())
            .unwrap_or_default()
    }

    pub(crate) fn package_local_nicknames_for(&self, name: &str) -> Vec<(String, String)> {
        let name = self.canonical_package_name(name);
        let mut nicknames = self
            .packages
            .get(&name)
            .map(|package| {
                package
                    .local_nicknames
                    .iter()
                    .map(|(nickname, target)| (nickname.clone(), target.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        nicknames.sort_by(|left, right| left.0.cmp(&right.0));
        nicknames
    }

    pub(crate) fn package_locally_nicknamed_by_list_for(&self, name: &str) -> Vec<String> {
        let name = self.canonical_package_name(name);
        let mut packages = self
            .packages
            .iter()
            .filter_map(|(package_name, package)| {
                package
                    .local_nicknames
                    .values()
                    .any(|target| target == &name)
                    .then(|| package_name.clone())
            })
            .collect::<Vec<_>>();
        packages.sort();
        packages
    }

    pub(crate) fn package_shadowing_symbols_for(&self, name: &str) -> Vec<String> {
        let name = self.canonical_package_name(name);
        let mut symbols = self
            .packages
            .get(&name)
            .map(|package| {
                package
                    .shadows
                    .iter()
                    .map(|symbol| {
                        package
                            .imports
                            .get(symbol)
                            .cloned()
                            .unwrap_or_else(|| canonical_symbol_name(&name, symbol))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        symbols.sort();
        symbols
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

    pub(crate) fn symbol_exists_exact(&self, package: &str, name: &str) -> bool {
        let package = self.canonical_package_name(package);
        package == COMMON_LISP_PACKAGE || self.symbol_status_exact(&package, name).is_some()
    }

    pub(crate) fn exact_symbol_name(&self, package: &str, name: &str) -> Option<String> {
        let package = self.canonical_package_name(package);
        self.packages
            .get(&package)
            .and_then(|entry| entry.exact_symbols.contains(name).then(|| name.to_string()))
    }

    pub(crate) fn imported_symbol_name(&self, package: &str, name: &str) -> String {
        self.imported_symbol_for(package, name)
            .unwrap_or_else(|| canonical_symbol_name(package, name))
    }

    pub(crate) fn symbol_object_for(&self, package: &str, name: &str) -> Option<SymbolObject> {
        let package = self.canonical_package_name(package);
        let normalized_name = normalize_symbol_name(name);
        self.packages.get(&package).and_then(|entry| {
            entry
                .import_objects
                .get(&normalized_name)
                .cloned()
                .or_else(|| entry.symbol_objects.get(&normalized_name).cloned())
        })
    }

    pub(crate) fn exact_symbol_object_for(
        &self,
        package: &str,
        name: &str,
    ) -> Option<SymbolObject> {
        let package = self.canonical_package_name(package);
        self.packages
            .get(&package)
            .and_then(|entry| entry.exact_symbol_objects.get(name).cloned())
    }

    pub(crate) fn all_package_names(&self) -> Vec<String> {
        let mut names = self.packages.keys().cloned().collect::<Vec<_>>();
        names.sort();
        names
    }

    pub(crate) fn package_symbol_objects_for(
        &self,
        name: &str,
        external_only: bool,
    ) -> Vec<SymbolObject> {
        let name = self.canonical_package_name(name);
        let Some(package) = self.packages.get(&name) else {
            return Vec::new();
        };
        let mut symbols = package
            .symbol_objects
            .iter()
            .filter(|(symbol_name, _)| {
                !external_only || package.exports.contains(*symbol_name) || name == KEYWORD_PACKAGE
            })
            .map(|(_, symbol)| symbol.clone())
            .collect::<Vec<_>>();
        symbols.extend(package.import_objects.values().cloned());
        for used in &package.use_packages {
            if let Some(used_package) = self.packages.get(used) {
                symbols.extend(
                    used_package
                        .symbol_objects
                        .iter()
                        .filter(|(symbol_name, _)| used_package.exports.contains(*symbol_name))
                        .map(|(_, symbol)| symbol.clone()),
                );
            }
        }
        symbols.sort_by_key(|symbol| (symbol.package().name(), symbol.name().to_string()));
        symbols.dedup_by(|left, right| left.ptr_eq(right));
        symbols
    }

    pub(crate) fn all_symbol_objects(&self) -> Vec<SymbolObject> {
        let mut symbols = self
            .packages
            .values()
            .flat_map(|package| package.symbol_objects.values().cloned())
            .collect::<Vec<_>>();
        symbols.sort_by_key(|symbol| (symbol.package().name(), symbol.name().to_string()));
        symbols.dedup_by(|left, right| left.ptr_eq(right));
        symbols
    }
}

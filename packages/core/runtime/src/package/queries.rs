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

    pub(crate) fn set_package_documentation(
        &mut self,
        package: &str,
        documentation: Option<String>,
    ) -> bool {
        let package = self.canonical_package_name(package);
        let Some(entry) = self.packages.get_mut(&package) else {
            return false;
        };
        entry.documentation = documentation;
        true
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

    pub(crate) fn package_nicknames(&self, name: &str) -> Vec<String> {
        let name = self.canonical_package_name(name);
        let mut nicknames = self
            .nicknames
            .iter()
            .filter_map(|(nickname, package)| (package == &name).then_some(nickname.clone()))
            .collect::<Vec<_>>();
        nicknames.sort();
        nicknames
    }

    pub(crate) fn shadowing_symbols_for(&self, name: &str) -> Vec<SymbolReference> {
        let name = self.canonical_package_name(name);
        let mut symbols = self
            .packages
            .get(&name)
            .map(|package| {
                package
                    .shadows
                    .iter()
                    .map(|symbol| SymbolReference::new(name.clone(), symbol.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        symbols.sort_by_key(SymbolReference::canonical_name);
        symbols
    }

    pub(crate) fn used_by_packages_for(&self, name: &str) -> Vec<String> {
        let name = self.canonical_package_name(name);
        let mut packages = self
            .packages
            .iter()
            .filter_map(|(package, entry)| {
                entry
                    .use_packages
                    .iter()
                    .any(|used| used == &name)
                    .then_some(package.clone())
            })
            .collect::<Vec<_>>();
        packages.sort();
        packages
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

    fn package_symbol_references(&self, name: &str, external_only: bool) -> Vec<SymbolReference> {
        let name = self.canonical_package_name(name);
        let Some(entry) = self.packages.get(&name) else {
            return Vec::new();
        };
        let mut references = entry
            .symbols
            .iter()
            .filter(|symbol| {
                !external_only
                    || name == COMMON_LISP_PACKAGE
                    || name == KEYWORD_PACKAGE
                    || entry.exports.contains(*symbol)
            })
            .map(|symbol| {
                entry
                    .imports
                    .get(symbol)
                    .and_then(|source| split_symbol(source))
                    .map(|(package, name, _)| {
                        SymbolReference::new(
                            self.canonical_package_name(package),
                            normalize_symbol_name(name),
                        )
                    })
                    .unwrap_or_else(|| SymbolReference::new(name.clone(), symbol.clone()))
            })
            .collect::<Vec<_>>();
        references.sort_by_key(SymbolReference::canonical_name);
        references.dedup();
        references
    }
}

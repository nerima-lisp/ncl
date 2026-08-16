use std::collections::{HashMap, HashSet};

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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SymbolReference {
    package: String,
    name: String,
}

impl SymbolReference {
    fn new(package: String, name: String) -> Self {
        Self { package, name }
    }

    pub(crate) fn package(&self) -> &str {
        &self.package
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn canonical_name(&self) -> String {
        format!("{}::{}", self.package, self.name)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PackageState {
    current: String,
    packages: HashMap<String, Package>,
    nicknames: HashMap<String, String>,
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
        }
    }

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

    pub(crate) fn use_package(&mut self, package: &str, target: &str) -> Result<(), String> {
        let package = self.canonical_package_name(package);
        let target = self.canonical_package_name(target);
        if !self.package_exists(&package) {
            return Err(format!("unknown package {package}"));
        }
        if !self.package_exists(&target) {
            return Err(format!("unknown package {target}"));
        }
        let Some(target_entry) = self.packages.get(&target) else {
            return Err(format!("unknown package {target}"));
        };
        if target_entry
            .use_packages
            .iter()
            .any(|used| used == &package)
        {
            return Ok(());
        }
        let target_shadows = target_entry.shadows.clone();
        let target_use_packages = target_entry.use_packages.clone();
        let mut accessible = self.package_symbol_references(&target, false);
        for used_package in target_use_packages {
            accessible.extend(self.package_symbol_references(&used_package, true));
        }
        for source_reference in self.package_symbol_references(&package, true) {
            if target_shadows.contains(source_reference.name()) {
                continue;
            }
            if accessible.iter().any(|existing| {
                existing.name() == source_reference.name() && existing != &source_reference
            }) {
                return Err(format!(
                    "name conflict while using {package} in {target}: {}",
                    source_reference.name()
                ));
            }
        }
        if let Some(entry) = self.packages.get_mut(&target) {
            entry.use_packages.push(package);
        }
        Ok(())
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
            entry.symbols.remove(&name)
                || entry.exports.remove(&name)
                || entry.imports.remove(&name).is_some()
                || entry.shadows.remove(&name)
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

    pub(crate) fn make_package(
        &mut self,
        name: String,
        nicknames: Vec<String>,
        use_packages: Vec<String>,
        documentation: Option<String>,
    ) -> Result<String, String> {
        let name = normalize_package_name(&name);
        if name.is_empty() {
            return Err("package name must not be empty".to_string());
        }
        if self.package_exists(&name) || self.nicknames.contains_key(&name) {
            return Err(format!("package {name} already exists"));
        }
        for package in &use_packages {
            let package = self.canonical_package_name(package);
            if !self.package_exists(&package) {
                return Err(format!("unknown package {package}"));
            }
        }
        self.define_package(
            name.clone(),
            nicknames,
            use_packages,
            HashSet::new(),
            documentation,
            HashMap::new(),
        )?;
        Ok(name)
    }

    pub(crate) fn delete_package(&mut self, name: &str) -> Result<bool, String> {
        let name = self.canonical_package_name(name);
        if name == COMMON_LISP_PACKAGE || name == KEYWORD_PACKAGE {
            return Err(format!("cannot delete package {name}"));
        }
        if self.packages.remove(&name).is_none() {
            return Err(format!("unknown package {name}"));
        }
        self.nicknames.retain(|_, package| package != &name);
        for package in self.packages.values_mut() {
            package.use_packages.retain(|used| used != &name);
            package.local_nicknames.retain(|_, target| target != &name);
            package
                .imports
                .retain(|_, source| !source.strip_prefix(&format!("{name}::")).is_some());
        }
        if self.current == name {
            self.current = if self.packages.contains_key(DEFAULT_PACKAGE) {
                DEFAULT_PACKAGE.to_string()
            } else {
                COMMON_LISP_PACKAGE.to_string()
            };
        }
        Ok(true)
    }

    pub(crate) fn rename_package(
        &mut self,
        name: &str,
        new_name: String,
        nicknames: Vec<String>,
    ) -> Result<String, String> {
        let old_name = self.canonical_package_name(name);
        if old_name == COMMON_LISP_PACKAGE || old_name == KEYWORD_PACKAGE {
            return Err(format!("cannot rename package {old_name}"));
        }
        let new_name = normalize_package_name(&new_name);
        if new_name.is_empty() {
            return Err("package name must not be empty".to_string());
        }
        if new_name != old_name
            && (self.packages.contains_key(&new_name) || self.nicknames.contains_key(&new_name))
        {
            return Err(format!("package {new_name} already exists"));
        }
        let mut normalized_nicknames = Vec::new();
        for nickname in nicknames {
            let nickname = normalize_package_name(&nickname);
            if nickname.is_empty() || nickname == new_name {
                return Err(format!("invalid package nickname {nickname}"));
            }
            if self.packages.contains_key(&nickname) && nickname != old_name {
                return Err(format!(
                    "package nickname {nickname} conflicts with an existing package"
                ));
            }
            if let Some(existing) = self.nicknames.get(&nickname)
                && existing != &old_name
            {
                return Err(format!("package nickname {nickname} is already in use"));
            }
            if !normalized_nicknames.contains(&nickname) {
                normalized_nicknames.push(nickname);
            }
        }
        let Some(mut package) = self.packages.remove(&old_name) else {
            return Err(format!("unknown package {old_name}"));
        };
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
            for source in entry.imports.values_mut() {
                if source.starts_with(&format!("{old_name}::")) {
                    *source = source.replacen(&old_name, &new_name, 1);
                }
            }
        }
        package.use_packages = package
            .use_packages
            .into_iter()
            .map(|used| {
                if used == old_name {
                    new_name.clone()
                } else {
                    used
                }
            })
            .collect();
        self.packages.insert(new_name.clone(), package);
        for nickname in normalized_nicknames {
            self.nicknames.insert(nickname, new_name.clone());
        }
        if self.current == old_name {
            self.current = new_name.clone();
        }
        Ok(new_name)
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
        let name = normalize_package_name(&name);
        if self.nicknames.contains_key(&name) {
            return Err(format!(
                "package name {name} conflicts with an existing nickname"
            ));
        }
        let mut normalized_nicknames = Vec::new();
        for nickname in nicknames {
            let nickname = normalize_package_name(&nickname);
            if nickname.is_empty() || nickname == name {
                return Err(format!("invalid package nickname {nickname}"));
            }
            if self.packages.contains_key(&nickname) {
                return Err(format!(
                    "package nickname {nickname} conflicts with an existing package"
                ));
            }
            if let Some(existing) = self.nicknames.get(&nickname)
                && existing != &name
            {
                return Err(format!("package nickname {nickname} is already in use"));
            }
            if !normalized_nicknames.contains(&nickname) {
                normalized_nicknames.push(nickname);
            }
        }
        let use_packages = use_packages
            .into_iter()
            .map(|package| self.canonical_package_name(&package))
            .collect();
        let exports: HashSet<String> = exports
            .into_iter()
            .map(|symbol| normalize_symbol_name(&symbol))
            .collect();
        let mut normalized_local_nicknames = HashMap::new();
        for (nickname, target) in local_nicknames {
            let nickname = normalize_package_name(&nickname);
            if nickname.is_empty() || nickname == name {
                return Err(format!("invalid local package nickname {nickname}"));
            }
            let target = self.canonical_package_name(&target);
            if !self.package_exists(&target) {
                return Err(format!(
                    "unknown package {target} for local nickname {nickname}"
                ));
            }
            if normalized_local_nicknames
                .insert(nickname.clone(), target)
                .is_some()
            {
                return Err(format!("duplicate local package nickname {nickname}"));
            }
        }

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
                local_nicknames: normalized_local_nicknames,
            },
        );
        for nickname in normalized_nicknames {
            self.nicknames.insert(nickname, name.clone());
        }
        Ok(())
    }

    pub(crate) fn set_current(&mut self, name: String) {
        self.current = self.canonical_package_name(&name);
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

pub(crate) fn normalize_symbol_name(name: &str) -> String {
    name.to_ascii_uppercase()
}

pub(crate) fn canonical_symbol_name(package: &str, name: &str) -> String {
    let package = normalize_package_name(package);
    let name = normalize_symbol_name(name);
    if package == DEFAULT_PACKAGE {
        name
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

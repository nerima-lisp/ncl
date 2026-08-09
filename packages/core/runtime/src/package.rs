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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SymbolResolutionError {
    Invalid,
    UnknownPackage(String),
    NotExported { package: String, name: String },
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

    pub(crate) fn package_exists(&self, name: &str) -> bool {
        self.packages.contains_key(&self.canonical_package_name(name))
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

    pub(crate) fn resolve_symbol(
        &self,
        raw: &str,
        current: &str,
    ) -> Result<SymbolReference, SymbolResolutionError> {
        if let Some((package_name, symbol_name, external)) = split_symbol(raw) {
            let package_name = self.canonical_package_name(package_name);
            let symbol_name = normalize_symbol_name(symbol_name);
            if package_name.is_empty() || symbol_name.is_empty() {
                return Err(SymbolResolutionError::Invalid);
            }
            if !self.package_exists(&package_name) {
                return Err(SymbolResolutionError::UnknownPackage(package_name));
            }
            if external && !self.is_exported(&package_name, &symbol_name) {
                return Err(SymbolResolutionError::NotExported {
                    package: package_name,
                    name: symbol_name,
                });
            }
            return Ok(SymbolReference::new(package_name, symbol_name));
        }

        let package_name = self.canonical_package_name(current);
        let symbol_name = normalize_symbol_name(raw);
        if package_name.is_empty() || symbol_name.is_empty() {
            return Err(SymbolResolutionError::Invalid);
        }
        if !self.package_exists(&package_name) {
            return Err(SymbolResolutionError::UnknownPackage(package_name));
        }
        Ok(SymbolReference::new(package_name, symbol_name))
    }

    pub(crate) fn use_packages_for(&self, name: &str) -> Vec<String> {
        let name = self.canonical_package_name(name);
        self.packages
            .get(&name)
            .map(|package| package.use_packages.clone())
            .unwrap_or_default()
    }

    pub(crate) fn use_package(&mut self, package: &str, target: &str) {
        let package = self.canonical_package_name(package);
        let target = self.canonical_package_name(target);
        if let Some(entry) = self.packages.get_mut(&target) {
            if !entry.use_packages.iter().any(|used| used == &package) {
                entry.use_packages.push(package);
            }
        }
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
            entry
                .imports
                .insert(source_name.clone(), canonical_symbol_name(&source_package, &source_name));
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
            let existed = entry.symbols.remove(&name)
                || entry.exports.remove(&name)
                || entry.imports.remove(&name).is_some()
                || entry.shadows.remove(&name);
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

    pub(crate) fn define_package(
        &mut self,
        name: String,
        nicknames: Vec<String>,
        use_packages: Vec<String>,
        exports: HashSet<String>,
    ) -> Result<(), String> {
        let name = normalize_package_name(&name);
        if self.nicknames.contains_key(&name) {
            return Err(format!("package name {name} conflicts with an existing nickname"));
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
                return Err(format!(
                    "package nickname {nickname} is already in use"
                ));
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

        self.nicknames.retain(|_, package| package != &name);
        self.packages.insert(
            name.clone(),
            Package {
                use_packages,
                symbols: exports.clone(),
                exports,
                imports: HashMap::new(),
                shadows: HashSet::new(),
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

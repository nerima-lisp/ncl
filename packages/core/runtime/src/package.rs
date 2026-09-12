use std::collections::{HashMap, HashSet};

use crate::value::{PackageObject, SymbolObject};

mod define;
mod lifecycle;
mod mutations;
mod names;
mod queries;
#[cfg(test)]
mod tests;

pub use names::{
    canonical_exact_symbol_name, canonical_symbol_name, normalize_package_name,
    normalize_symbol_name, split_symbol,
};

pub const DEFAULT_PACKAGE: &str = "NCL-USER";
pub const COMMON_LISP_PACKAGE: &str = "COMMON-LISP";
pub const KEYWORD_PACKAGE: &str = "KEYWORD";

#[derive(Clone, Debug)]
struct Package {
    object: PackageObject,
    use_packages: Vec<String>,
    nicknames: Vec<String>,
    exports: HashSet<String>,
    symbols: HashSet<String>,
    exact_symbols: HashSet<String>,
    imports: HashMap<String, String>,
    symbol_objects: HashMap<String, SymbolObject>,
    exact_symbol_objects: HashMap<String, SymbolObject>,
    import_objects: HashMap<String, SymbolObject>,
    shadows: HashSet<String>,
    documentation: Option<String>,
    local_nicknames: HashMap<String, String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolStatus {
    Internal,
    External,
}

#[derive(Clone, Debug)]
pub struct PackageState {
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
                object: PackageObject::new(COMMON_LISP_PACKAGE),
                use_packages: Vec::new(),
                nicknames: Vec::new(),
                exports: HashSet::new(),
                symbols: HashSet::new(),
                exact_symbols: HashSet::new(),
                imports: HashMap::new(),
                symbol_objects: HashMap::new(),
                exact_symbol_objects: HashMap::new(),
                import_objects: HashMap::new(),
                shadows: HashSet::new(),
                documentation: None,
                local_nicknames: HashMap::new(),
            },
        );
        packages.insert(
            DEFAULT_PACKAGE.to_string(),
            Package {
                object: PackageObject::new(DEFAULT_PACKAGE),
                use_packages: vec![COMMON_LISP_PACKAGE.to_string()],
                nicknames: Vec::new(),
                exports: HashSet::new(),
                symbols: HashSet::new(),
                exact_symbols: HashSet::new(),
                imports: HashMap::new(),
                symbol_objects: HashMap::new(),
                exact_symbol_objects: HashMap::new(),
                import_objects: HashMap::new(),
                shadows: HashSet::new(),
                documentation: None,
                local_nicknames: HashMap::new(),
            },
        );
        packages.insert(
            KEYWORD_PACKAGE.to_string(),
            Package {
                object: PackageObject::new(KEYWORD_PACKAGE),
                use_packages: Vec::new(),
                nicknames: Vec::new(),
                exports: HashSet::new(),
                symbols: HashSet::new(),
                exact_symbols: HashSet::new(),
                imports: HashMap::new(),
                symbol_objects: HashMap::new(),
                exact_symbol_objects: HashMap::new(),
                import_objects: HashMap::new(),
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
}

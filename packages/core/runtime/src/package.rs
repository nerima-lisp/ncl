use std::collections::{HashMap, HashSet};

mod define;
mod mutations;
mod names;
mod queries;

pub use names::{
    canonical_symbol_name, normalize_package_name, normalize_symbol_name, split_symbol,
};

pub const DEFAULT_PACKAGE: &str = "NCL-USER";
pub const COMMON_LISP_PACKAGE: &str = "COMMON-LISP";
pub const KEYWORD_PACKAGE: &str = "KEYWORD";

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
}

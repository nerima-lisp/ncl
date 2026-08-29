use std::collections::{HashMap, HashSet};

use crate::package;

pub(super) struct DefpackageSpec {
    pub(super) name: String,
    pub(super) nicknames: Vec<String>,
    pub(super) use_packages: Vec<String>,
    pub(super) exports: HashSet<String>,
    pub(super) operations: Vec<DefpackageOperation>,
    pub(super) documentation: Option<String>,
    pub(super) local_nicknames: HashMap<String, String>,
}

pub(super) enum DefpackageOperation {
    Shadow(String),
    Intern(String),
    Import {
        source_package: String,
        source_name: String,
        shadowing: bool,
    },
}

/// Accumulates `DEFPACKAGE` option state across a single left-to-right pass
/// over the option forms, before being converted into a [`DefpackageSpec`].
pub(super) struct DefpackageBuilder {
    pub(super) nicknames: Vec<String>,
    pub(super) use_packages: Vec<String>,
    pub(super) exports: HashSet<String>,
    pub(super) operations: Vec<DefpackageOperation>,
    pub(super) documentation: Option<String>,
    pub(super) local_nicknames: HashMap<String, String>,
    pub(super) saw_nicknames: bool,
    pub(super) saw_use: bool,
    pub(super) saw_documentation: bool,
    pub(super) saw_size: bool,
}

impl DefpackageBuilder {
    pub(super) fn new() -> Self {
        Self {
            nicknames: Vec::new(),
            use_packages: vec![package::COMMON_LISP_PACKAGE.to_string()],
            exports: HashSet::new(),
            operations: Vec::new(),
            documentation: None,
            local_nicknames: HashMap::new(),
            saw_nicknames: false,
            saw_use: false,
            saw_documentation: false,
            saw_size: false,
        }
    }

    pub(super) fn into_spec(self, name: String) -> DefpackageSpec {
        DefpackageSpec {
            name,
            nicknames: self.nicknames,
            use_packages: self.use_packages,
            exports: self.exports,
            operations: self.operations,
            documentation: self.documentation,
            local_nicknames: self.local_nicknames,
        }
    }
}

impl Default for DefpackageBuilder {
    fn default() -> Self {
        Self::new()
    }
}

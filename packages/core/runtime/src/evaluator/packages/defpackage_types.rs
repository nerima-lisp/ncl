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
///
/// The `saw_*` flags each independently guard against one `DEFPACKAGE`
/// clause repeating, which is why there are four of them rather than one
/// combined state.
#[expect(clippy::struct_excessive_bools)]
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

#[cfg(test)]
mod tests {
    use super::DefpackageBuilder;

    #[test]
    fn default_builder_matches_a_freshly_constructed_builder() {
        let default_builder = DefpackageBuilder::default();
        let new_builder = DefpackageBuilder::new();

        assert_eq!(default_builder.use_packages, new_builder.use_packages);
        assert!(!default_builder.saw_nicknames);
        assert!(!default_builder.saw_use);
        assert!(!default_builder.saw_documentation);
        assert!(!default_builder.saw_size);
        assert!(default_builder.nicknames.is_empty());
        assert!(default_builder.local_nicknames.is_empty());
    }
}

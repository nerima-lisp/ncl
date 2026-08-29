use super::names::{normalize_package_name, normalize_symbol_name};
use super::{Package, PackageState};
use std::collections::{HashMap, HashSet};

impl PackageState {
    pub(crate) fn define_package(
        &mut self,
        name: &str,
        nicknames: Vec<String>,
        use_packages: Vec<String>,
        exports: HashSet<String>,
        documentation: Option<String>,
        local_nicknames: HashMap<String, String>,
    ) -> Result<(), String> {
        let name = normalize_package_name(name);
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
}

use super::names::normalize_package_name;
use super::{COMMON_LISP_PACKAGE, DEFAULT_PACKAGE, KEYWORD_PACKAGE, PackageState};

fn protected_package(name: &str) -> bool {
    matches!(name, COMMON_LISP_PACKAGE | KEYWORD_PACKAGE)
}

impl PackageState {
    pub(crate) fn rename_package(
        &mut self,
        old_name: &str,
        new_name: &str,
        nicknames: Vec<String>,
    ) -> Result<String, String> {
        let old_name = self.canonical_package_name(old_name);
        let new_name = normalize_package_name(new_name);
        if new_name.is_empty() {
            return Err("package name cannot be empty".to_string());
        }
        if protected_package(&old_name) {
            return Err(format!("package {old_name} is locked"));
        }
        if protected_package(&new_name) && new_name != old_name {
            return Err(format!("package {new_name} is locked"));
        }
        if !self.packages.contains_key(&old_name) {
            return Err(format!("unknown package {old_name}"));
        }
        if let Some(existing) = self.packages.get(&new_name)
            && new_name != old_name
        {
            let _ = existing;
            return Err(format!("package {new_name} is already in use"));
        }
        if let Some(existing) = self.nicknames.get(&new_name)
            && existing != &old_name
        {
            return Err(format!(
                "package name {new_name} conflicts with an existing nickname"
            ));
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

        let mut package = self
            .packages
            .remove(&old_name)
            .expect("package existence checked above");
        self.nicknames
            .retain(|_, package_name| package_name != &old_name);

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
            for imported in entry.imports.values_mut() {
                rename_symbol_package(imported, &old_name, &new_name);
            }
        }
        for target in package.local_nicknames.values_mut() {
            if target == &old_name {
                *target = new_name.clone();
            }
        }
        for imported in package.imports.values_mut() {
            rename_symbol_package(imported, &old_name, &new_name);
        }

        package.object.rename(new_name.clone());
        package.nicknames = normalized_nicknames.clone();
        self.packages.insert(new_name.clone(), package);
        for nickname in normalized_nicknames {
            self.nicknames.insert(nickname, new_name.clone());
        }
        if self.current == old_name {
            self.current = new_name.clone();
        }
        Ok(new_name)
    }

    pub(crate) fn delete_package(&mut self, name: &str) -> Result<(), String> {
        let name = self.canonical_package_name(name);
        if protected_package(&name) {
            return Err(format!("package {name} is locked"));
        }
        if !self.packages.contains_key(&name) {
            return Err(format!("unknown package {name}"));
        }
        if self.packages.values().any(|package| {
            package
                .use_packages
                .iter()
                .any(|used_package| used_package == &name)
        }) {
            return Err(format!("package {name} is used by another package"));
        }

        let package = self
            .packages
            .remove(&name)
            .expect("package existence checked above");
        self.nicknames
            .retain(|_, package_name| package_name != &name);
        for entry in self.packages.values_mut() {
            entry.local_nicknames.retain(|_, target| target != &name);
        }
        package.object.delete();
        if self.current == name {
            self.current = DEFAULT_PACKAGE.to_string();
        }
        Ok(())
    }
}

fn rename_symbol_package(symbol: &mut String, old_name: &str, new_name: &str) {
    let Some((package, symbol_name)) = symbol.split_once("::") else {
        return;
    };
    if package == old_name {
        *symbol = format!("{new_name}::{symbol_name}");
    }
}

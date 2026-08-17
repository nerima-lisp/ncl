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

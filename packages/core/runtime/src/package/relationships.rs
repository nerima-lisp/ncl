impl PackageState {
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
}

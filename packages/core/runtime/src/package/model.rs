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

use std::collections::{HashMap, HashSet};

use super::{PackageState, normalize_symbol_name};

pub(super) struct PreparedPackage {
    pub(super) name: String,
    pub(super) nicknames: Vec<String>,
    pub(super) use_packages: Vec<String>,
    pub(super) exports: HashSet<String>,
    pub(super) documentation: Option<String>,
    pub(super) local_nicknames: HashMap<String, String>,
}

pub(super) fn prepare(
    state: &PackageState,
    name: String,
    nicknames: Vec<String>,
    use_packages: Vec<String>,
    exports: HashSet<String>,
    documentation: Option<String>,
    local_nicknames: HashMap<String, String>,
) -> Result<PreparedPackage, String> {
    if state.nicknames.contains_key(&name) {
        return Err(format!(
            "package name {name} conflicts with an existing nickname"
        ));
    }

    let mut normalized_nicknames = Vec::new();
    for nickname in nicknames {
        if nickname.is_empty() || nickname == name {
            return Err(format!("invalid package nickname {nickname}"));
        }
        if state.packages.contains_key(&nickname) {
            return Err(format!(
                "package nickname {nickname} conflicts with an existing package"
            ));
        }
        if let Some(existing) = state.nicknames.get(&nickname)
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
        .map(|package| state.canonical_package_name(&package))
        .collect::<Vec<_>>();
    if let Some(package) = use_packages
        .iter()
        .find(|package| !state.package_exists(package))
    {
        return Err(format!("unknown package {package}"));
    }
    let exports = exports
        .into_iter()
        .map(|symbol| normalize_symbol_name(&symbol))
        .collect();
    let mut normalized_local_nicknames = HashMap::new();
    for (nickname, target) in local_nicknames {
        if nickname.is_empty() || nickname == name {
            return Err(format!("invalid local package nickname {nickname}"));
        }
        let target = state.canonical_package_name(&target);
        if !state.package_exists(&target) {
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

    Ok(PreparedPackage {
        name,
        nicknames: normalized_nicknames,
        use_packages,
        exports,
        documentation,
        local_nicknames: normalized_local_nicknames,
    })
}

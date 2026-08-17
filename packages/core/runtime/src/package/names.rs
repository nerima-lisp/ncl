pub(crate) fn normalize_package_name(name: &str) -> String {
    let name = name.strip_prefix(':').unwrap_or(name);
    let name = name.to_ascii_uppercase();
    if name == "CL" {
        COMMON_LISP_PACKAGE.to_string()
    } else {
        name
    }
}

pub(crate) fn normalize_symbol_name(name: &str) -> String {
    name.to_ascii_uppercase()
}

pub(crate) fn canonical_symbol_name(package: &str, name: &str) -> String {
    let package = normalize_package_name(package);
    let name = normalize_symbol_name(name);
    if package == DEFAULT_PACKAGE {
        name
    } else {
        format!("{package}::{name}")
    }
}

pub(crate) fn split_symbol(name: &str) -> Option<(&str, &str, bool)> {
    if let Some((package, symbol)) = name.split_once("::") {
        return Some((package, symbol, false));
    }
    name.split_once(':')
        .map(|(package, symbol)| (package, symbol, true))
}

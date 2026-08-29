use super::{COMMON_LISP_PACKAGE, DEFAULT_PACKAGE};

pub fn normalize_package_name(name: &str) -> String {
    let name = name.strip_prefix(':').unwrap_or(name);
    let name = name.to_ascii_uppercase();
    if name == "CL" {
        COMMON_LISP_PACKAGE.to_string()
    } else {
        name
    }
}

pub fn normalize_symbol_name(name: &str) -> String {
    name.to_ascii_uppercase()
}

pub fn canonical_symbol_name(package: &str, name: &str) -> String {
    let package = normalize_package_name(package);
    let name = normalize_symbol_name(name);
    if package == DEFAULT_PACKAGE {
        name
    } else {
        format!("{package}::{name}")
    }
}

pub fn split_symbol(name: &str) -> Option<(&str, &str, bool)> {
    if let Some((package, symbol)) = name.split_once("::") {
        return Some((package, symbol, false));
    }
    name.split_once(':')
        .map(|(package, symbol)| (package, symbol, true))
}

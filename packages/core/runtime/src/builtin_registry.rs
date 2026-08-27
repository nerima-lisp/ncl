#![allow(clippy::wildcard_imports)]
use super::*;

#[path = "builtin_constants.rs"]
mod builtin_constants;
use builtin_constants::constant_bindings;
#[path = "builtin_definitions.rs"]
mod builtin_definitions;
use builtin_definitions::{BUILTIN_DEFINITIONS, PRIMITIVE_NAMES};

pub fn install(environment: &Environment) {
    install_builtins(environment);
    install_primitives(environment);
    install_constants(environment);
}

fn install_builtins(environment: &Environment) {
    for (name, function) in BUILTIN_DEFINITIONS {
        let value = Value::builtin(name, *function);
        let normalized = normalize_name(name);
        environment.define(normalized.clone(), value.clone());
        environment.define(format!("{COMMON_LISP_PACKAGE}::{normalized}"), value);
    }
}

fn install_primitives(environment: &Environment) {
    for name in PRIMITIVE_NAMES {
        let value = Value::primitive(name);
        environment.define(name, value.clone());
        environment.define(format!("{COMMON_LISP_PACKAGE}::{name}"), value);
    }
}

fn install_constants(environment: &Environment) {
    for (name, value) in constant_bindings() {
        environment.define(name, value.clone());
        environment.define(format!("{COMMON_LISP_PACKAGE}::{name}"), value);
    }
}

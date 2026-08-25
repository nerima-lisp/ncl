use crate::{Environment, Value};

use super::COMMON_LISP_PACKAGE;

pub(super) fn install_constants(environment: &Environment) {
    for (name, value) in [
        ("NIL", Value::Nil),
        ("T", Value::boolean(true)),
        ("CHAR-CODE-LIMIT", Value::Integer(0x11_00_00)),
        ("MOST-POSITIVE-CHAR-CODE", Value::Integer(0x10_FF_FF)),
    ] {
        environment.define(name, value.clone());
        environment.define(format!("{COMMON_LISP_PACKAGE}::{name}"), value);
    }
}

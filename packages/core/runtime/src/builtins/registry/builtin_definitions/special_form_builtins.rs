#![allow(clippy::wildcard_imports)]
use super::*;

pub(super) const SPECIAL_FORM_BUILTINS: &[BuiltinDefinition] = &[
    (
        "simple-condition-format-control",
        simple_condition_format_control as _,
    ),
    (
        "simple-condition-format-arguments",
        simple_condition_format_arguments as _,
    ),
    ("__NCL_THE_CHECK", the_check as _),
    ("__NCL_ECASE_ERROR", ecase_error as _),
    ("__NCL_ETYPECASE_ERROR", etypecase_error as _),
];

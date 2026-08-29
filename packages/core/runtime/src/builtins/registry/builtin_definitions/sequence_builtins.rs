#![allow(clippy::wildcard_imports)]
use super::*;

pub(super) const SEQUENCE_BUILTINS: &[BuiltinDefinition] = &[
    ("nth", nth as _),
    ("elt", elt as _),
    ("subseq", subseq as _),
    ("fill", fill as _),
    ("replace", replace as _),
    ("copy-seq", copy_seq as _),
    ("concatenate", concatenate as _),
    ("coerce", coerce as _),
];

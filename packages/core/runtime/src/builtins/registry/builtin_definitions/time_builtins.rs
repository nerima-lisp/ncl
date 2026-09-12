#![allow(clippy::wildcard_imports)]
use super::*;

pub(super) const TIME_BUILTINS: &[BuiltinDefinition] = &[("sleep", sleep as _)];

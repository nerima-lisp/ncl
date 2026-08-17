use std::collections::{HashMap, HashSet};

pub(crate) const DEFAULT_PACKAGE: &str = "NCL-USER";
pub(crate) const COMMON_LISP_PACKAGE: &str = "COMMON-LISP";
pub(crate) const KEYWORD_PACKAGE: &str = "KEYWORD";

include!("package/model.rs");
include!("package/queries.rs");
include!("package/symbols.rs");
include!("package/relationships.rs");
include!("package/lifecycle.rs");
include!("package/names.rs");

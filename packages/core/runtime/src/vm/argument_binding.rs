//! Lambda-list argument binding: turning call-site arguments into the
//! required, optional, rest, keyword, and auxiliary bindings a compiled
//! function body expects.

mod keywords;
mod positional;
pub(super) mod support;

use crate::Environment;

pub(super) struct BindingContext<'a> {
    pub(super) local: Environment,
    pub(super) span: ncl_syntax::Span,
    pub(super) special_names: &'a [(String, bool)],
}

impl<'a> BindingContext<'a> {
    pub(super) fn new(
        local: &Environment,
        span: ncl_syntax::Span,
        special_names: &'a [(String, bool)],
    ) -> Self {
        Self {
            local: local.clone(),
            span,
            special_names,
        }
    }
}

pub(super) fn declare_special_if(
    local: &Environment,
    name: &str,
    escaped: bool,
    special_names: &[(String, bool)],
) {
    if !special_names
        .iter()
        .any(|(declared_name, declared_escaped)| {
            declared_name == name && *declared_escaped == escaped
        })
    {
        return;
    }
    if escaped {
        local.declare_special_exact(name);
    } else {
        local.declare_special(name);
    }
}

pub use keywords::{bind_auxiliary, bind_keywords};
pub use positional::{argument_layout, bind_optional, bind_required, bind_rest};

#[cfg(test)]
mod tests;

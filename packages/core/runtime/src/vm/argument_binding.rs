//! Lambda-list argument binding: turning call-site arguments into the
//! required, optional, rest, keyword, and auxiliary bindings a compiled
//! function body expects.

mod keywords;
mod positional;
mod support;

pub use keywords::{bind_auxiliary, bind_keywords};
pub use positional::{argument_layout, bind_optional, bind_required, bind_rest};

#[cfg(test)]
mod tests;

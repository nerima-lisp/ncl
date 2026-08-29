use ncl_syntax::{Form, FormKind, Span};

use crate::{Runtime, RuntimeError};

use super::defpackage_types::DefpackageBuilder;

impl Runtime {
    /// Applies a `DEFPACKAGE` option that configures package identity or
    /// naming metadata (`:nicknames`, `:use`, `:documentation`, `:size`,
    /// `:local-nicknames`). Returns `Ok(false)` when `normalized_option`
    /// names a different kind of option, so the caller can try the next
    /// handler.
    pub(super) fn apply_defpackage_metadata_option(
        builder: &mut DefpackageBuilder,
        normalized_option: &str,
        option_items: &[Form],
        option_span: Span,
    ) -> Result<bool, RuntimeError> {
        match normalized_option {
            "NICKNAMES" => {
                if builder.saw_nicknames {
                    return Err(Self::invalid(
                        "defpackage has duplicate :nicknames options",
                        option_span,
                    ));
                }
                builder.saw_nicknames = true;
                for package_form in option_items.iter().skip(1) {
                    builder
                        .nicknames
                        .push(Self::package_name_from_form(package_form)?);
                }
            }
            "USE" => {
                if builder.saw_use {
                    return Err(Self::invalid(
                        "defpackage has duplicate :use options",
                        option_span,
                    ));
                }
                builder.saw_use = true;
                builder.use_packages.clear();
                for package_form in option_items.iter().skip(1) {
                    builder
                        .use_packages
                        .push(Self::package_name_from_form(package_form)?);
                }
            }
            "DOCUMENTATION" => {
                if builder.saw_documentation || option_items.len() != 2 {
                    return Err(Self::invalid(
                        "defpackage :documentation needs one string",
                        option_span,
                    ));
                }
                builder.saw_documentation = true;
                let FormKind::String(value) = &option_items[1].kind else {
                    return Err(Self::invalid(
                        "defpackage :documentation needs a string",
                        option_items[1].span,
                    ));
                };
                builder.documentation = Some(value.clone());
            }
            "SIZE" => {
                if builder.saw_size || option_items.len() != 2 {
                    return Err(Self::invalid(
                        "defpackage :size needs one non-negative integer",
                        option_span,
                    ));
                }
                builder.saw_size = true;
                let FormKind::Atom(value) = &option_items[1].kind else {
                    return Err(Self::invalid(
                        "defpackage :size needs a non-negative integer",
                        option_items[1].span,
                    ));
                };
                if value.parse::<i64>().map_or(true, |size| size < 0) {
                    return Err(Self::invalid(
                        "defpackage :size needs a non-negative integer",
                        option_items[1].span,
                    ));
                }
            }
            "LOCAL-NICKNAMES" => {
                for nickname_option in option_items.iter().skip(1) {
                    let FormKind::List(mapping) = &nickname_option.kind else {
                        return Err(Self::invalid(
                            "defpackage local nickname needs a two-element list",
                            nickname_option.span,
                        ));
                    };
                    if mapping.len() != 2 {
                        return Err(Self::invalid(
                            "defpackage local nickname needs a two-element list",
                            nickname_option.span,
                        ));
                    }
                    let nickname = Self::package_name_from_form(&mapping[0])?;
                    let target = Self::package_name_from_form(&mapping[1])?;
                    if builder.local_nicknames.insert(nickname, target).is_some() {
                        return Err(Self::invalid(
                            "defpackage has duplicate local package nickname",
                            nickname_option.span,
                        ));
                    }
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}

use std::collections::HashSet;

use ncl_syntax::{Form, FormKind};

use crate::environment::normalize_name;
use crate::evaluator::helpers::{atom_name, macro_keyword_name};
use crate::value::{MacroKeywordParameter, MacroPattern};
use crate::{Runtime, RuntimeError};

impl Runtime {
    pub(super) fn parse_macro_keyword_parameter(
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<MacroKeywordParameter, RuntimeError> {
        let nil = || Form::atom("NIL", form.span);
        let (keyword_name, pattern, trailing_start) = match &form.kind {
            FormKind::Atom(_) => {
                let name = Self::macro_binding_name(form, seen)?;
                let keyword_name = normalize_name(&name);
                (keyword_name, MacroPattern::Name(name), 0)
            }
            FormKind::List(items) if !items.is_empty() => {
                if let FormKind::List(key_specification) = &items[0].kind {
                    if key_specification.len() != 2 {
                        return Err(Self::invalid(
                            "macro keyword designator must contain a keyword and variable",
                            items[0].span,
                        ));
                    }
                    let Some(keyword_name) = macro_keyword_name(&key_specification[0]) else {
                        return Err(Self::invalid(
                            "macro keyword designator must start with a keyword",
                            key_specification[0].span,
                        ));
                    };
                    let pattern = Self::macro_pattern(&key_specification[1], seen)?;
                    (keyword_name, pattern, 1)
                } else if atom_name(&items[0]).is_some_and(|name| name.starts_with(':')) {
                    let Some(keyword_name) = macro_keyword_name(&items[0]) else {
                        return Err(Self::invalid(
                            "macro keyword designator must be a nonempty keyword",
                            items[0].span,
                        ));
                    };
                    if items.len() < 2 {
                        return Err(Self::invalid(
                            "macro keyword parameter needs a variable",
                            form.span,
                        ));
                    }
                    let pattern = Self::macro_pattern(&items[1], seen)?;
                    (keyword_name, pattern, 2)
                } else {
                    let pattern = Self::macro_pattern(&items[0], seen)?;
                    let MacroPattern::Name(name) = &pattern else {
                        return Err(Self::invalid(
                            "macro keyword parameter must have a variable name",
                            items[0].span,
                        ));
                    };
                    (normalize_name(name), pattern, 1)
                }
            }
            FormKind::List(_) => unreachable!(),
            _ => {
                return Err(Self::invalid(
                    "macro keyword parameter must be a symbol or list",
                    form.span,
                ));
            }
        };

        let item_count = match &form.kind {
            FormKind::Atom(_) => 0,
            FormKind::List(items) => items.len(),
            _ => unreachable!(),
        };
        if item_count > trailing_start + 2 {
            return Err(Self::invalid(
                "macro keyword parameter contains too many items",
                form.span,
            ));
        }
        let (init_form, supplied_p) = match &form.kind {
            FormKind::Atom(_) => (nil(), None),
            FormKind::List(items) => (
                items.get(trailing_start).cloned().unwrap_or_else(nil),
                items
                    .get(trailing_start + 1)
                    .map(|item| Self::macro_binding_name(item, seen))
                    .transpose()?,
            ),
            _ => unreachable!(),
        };
        Ok(MacroKeywordParameter {
            keyword_name,
            pattern,
            init_form,
            supplied_p,
        })
    }
}

use std::collections::HashSet;

use ncl_syntax::{Form, FormKind};

use crate::value::{MacroAuxiliaryParameter, MacroOptionalParameter};
use crate::{Runtime, RuntimeError};

impl Runtime {
    pub(super) fn parse_macro_optional_parameter(
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<MacroOptionalParameter, RuntimeError> {
        let nil = || Form::atom("NIL", form.span);
        match &form.kind {
            FormKind::Atom(_) => Ok(MacroOptionalParameter {
                pattern: Self::macro_pattern(form, seen)?,
                init_form: nil(),
                supplied_p: None,
            }),
            FormKind::List(items) if (1..=3).contains(&items.len()) => {
                let pattern = Self::macro_pattern(&items[0], seen)?;
                let init_form = items.get(1).cloned().unwrap_or_else(nil);
                let supplied_p = items
                    .get(2)
                    .map(|item| Self::macro_binding_name(item, seen))
                    .transpose()?;
                Ok(MacroOptionalParameter {
                    pattern,
                    init_form,
                    supplied_p,
                })
            }
            FormKind::List(_) => Err(Self::invalid(
                "macro optional parameter must contain one to three items",
                form.span,
            )),
            _ => Err(Self::invalid(
                "macro optional parameter must be a symbol or list",
                form.span,
            )),
        }
    }

    pub(super) fn parse_macro_auxiliary_parameter(
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<MacroAuxiliaryParameter, RuntimeError> {
        match &form.kind {
            FormKind::Atom(_) => Ok(MacroAuxiliaryParameter {
                name: Self::macro_binding_name(form, seen)?,
                init_form: Form::atom("NIL", form.span),
            }),
            FormKind::List(items) if (1..=2).contains(&items.len()) => {
                Ok(MacroAuxiliaryParameter {
                    name: Self::macro_binding_name(&items[0], seen)?,
                    init_form: items
                        .get(1)
                        .cloned()
                        .unwrap_or_else(|| Form::atom("NIL", form.span)),
                })
            }
            FormKind::List(_) => Err(Self::invalid(
                "macro auxiliary parameter must contain one or two items",
                form.span,
            )),
            _ => Err(Self::invalid(
                "macro auxiliary parameter must be a symbol or list",
                form.span,
            )),
        }
    }
}

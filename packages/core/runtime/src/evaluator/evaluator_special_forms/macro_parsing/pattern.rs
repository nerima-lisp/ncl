use std::collections::HashSet;

use ncl_syntax::{Form, FormKind};

use crate::environment::normalize_name;
use crate::evaluator::evaluator_literals::literal_atom;
use crate::evaluator::helpers::atom_name;
use crate::value::MacroPattern;
use crate::{Runtime, RuntimeError};

impl Runtime {
    pub(super) fn macro_binding_name(
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<String, RuntimeError> {
        let Some(name) = atom_name(form) else {
            return Err(Self::invalid("macro parameter must be a symbol", form.span));
        };
        let normalized = normalize_name(name);
        if normalized.is_empty()
            || normalized.starts_with('&')
            || literal_atom(name).is_some()
            || !seen.insert(normalized.clone())
        {
            return Err(Self::invalid(
                "macro parameter names must be unique and bindable",
                form.span,
            ));
        }
        Ok(normalized)
    }

    pub(in crate::evaluator::evaluator_special_forms) fn macro_pattern(
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<MacroPattern, RuntimeError> {
        match &form.kind {
            FormKind::Atom(_) => Ok(MacroPattern::Name(Self::macro_binding_name(form, seen)?)),
            FormKind::List(items) => Ok(MacroPattern::List(
                items
                    .iter()
                    .map(|item| Self::macro_pattern(item, seen))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            FormKind::DottedList { items, tail } => Ok(MacroPattern::Dotted {
                items: items
                    .iter()
                    .map(|item| Self::macro_pattern(item, seen))
                    .collect::<Result<Vec<_>, _>>()?,
                tail: Box::new(Self::macro_pattern(tail, seen)?),
            }),
            _ => Err(Self::invalid(
                "macro destructuring pattern must be a symbol or list",
                form.span,
            )),
        }
    }
}

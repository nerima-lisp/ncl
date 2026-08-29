use ncl_syntax::{Form, FormKind, parse_symbol_token};

use crate::evaluator::evaluator_literals::literal_atom;
use crate::evaluator::helpers::atom_name;
use crate::{Runtime, RuntimeError};

impl Runtime {
    pub(super) fn expand_builtin_with_slots(
        form: &Form,
        with_accessors: bool,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        let operator = if with_accessors {
            "WITH-ACCESSORS"
        } else {
            "WITH-SLOTS"
        };
        if items.len() < 3 {
            return Err(Self::arity(
                operator,
                "at least two",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(Self::invalid(
                if with_accessors {
                    "with-accessors bindings must be a list"
                } else {
                    "with-slots bindings must be a list"
                },
                items[1].span,
            ));
        };

        let temporary = Self::symbol_macro_temporary(&items[2], 0, form.span);
        let symbol_bindings = bindings
            .iter()
            .map(|entry| Self::expand_builtin_slot_binding(entry, &temporary, with_accessors))
            .collect::<Result<Vec<_>, _>>()?;

        let symbol_macrolet = {
            let mut forms = Vec::with_capacity(items.len().saturating_sub(1));
            forms.push(Form::atom("SYMBOL-MACROLET", form.span));
            forms.push(Form::list(symbol_bindings, items[1].span));
            forms.extend(items[3..].iter().cloned());
            Form::list(forms, form.span)
        };
        let let_bindings = Form::list(
            vec![Form::list(vec![temporary, items[2].clone()], items[2].span)],
            items[1].span,
        );
        Ok(Form::list(
            vec![Form::atom("LET", form.span), let_bindings, symbol_macrolet],
            form.span,
        ))
    }

    fn expand_builtin_slot_binding(
        entry: &Form,
        temporary: &Form,
        with_accessors: bool,
    ) -> Result<Form, RuntimeError> {
        let (variable, expansion) = if with_accessors {
            let FormKind::List(parts) = &entry.kind else {
                return Err(Self::invalid(
                    "with-accessors entry must be a (variable accessor) list",
                    entry.span,
                ));
            };
            if parts.len() != 2 {
                return Err(Self::invalid(
                    "with-accessors entry needs a variable and accessor",
                    entry.span,
                ));
            }
            Self::variable_name_info(&parts[0], "with-accessors variable must be a symbol")?;
            Self::validate_builtin_slot_symbol(
                &parts[1],
                "with-accessors accessor must be a symbol",
            )?;
            (
                parts[0].clone(),
                Form::list(vec![parts[1].clone(), temporary.clone()], entry.span),
            )
        } else {
            let (slot, variable) = match &entry.kind {
                FormKind::Atom(_) => (entry.clone(), entry.clone()),
                FormKind::List(parts) if parts.len() == 2 => (parts[0].clone(), parts[1].clone()),
                _ => {
                    return Err(Self::invalid(
                        "with-slots entry must be a slot or (slot variable) list",
                        entry.span,
                    ));
                }
            };
            Self::validate_builtin_slot_symbol(&slot, "with-slots slot must be a symbol")?;
            Self::variable_name_info(&variable, "with-slots variable must be a symbol")?;
            let quoted_slot = Form::list(vec![Form::atom("QUOTE", slot.span), slot], entry.span);
            (
                variable,
                Form::list(
                    vec![
                        Form::atom("SLOT-VALUE", entry.span),
                        temporary.clone(),
                        quoted_slot,
                    ],
                    entry.span,
                ),
            )
        };
        Ok(Form::list(vec![variable, expansion], entry.span))
    }

    fn validate_builtin_slot_symbol(candidate: &Form, context: &str) -> Result<(), RuntimeError> {
        let Some(name) = atom_name(candidate) else {
            return Err(Self::invalid(context, candidate.span));
        };
        let Ok(token) = parse_symbol_token(name) else {
            return Err(Self::invalid(context, candidate.span));
        };
        if token.name.is_empty()
            || (!token.escaped
                && literal_atom(name).is_some()
                && !name.eq_ignore_ascii_case("nil")
                && !name.eq_ignore_ascii_case("t"))
        {
            return Err(Self::invalid(context, candidate.span));
        }
        Ok(())
    }
}

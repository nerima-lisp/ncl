use ncl_syntax::{Form, FormKind};

use crate::evaluator::helpers::atom_name;
use crate::{Runtime, RuntimeError};

impl Runtime {
    pub(super) fn expand_builtin_symbol_iteration(
        form: &Form,
        external_only: bool,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        let operator = if external_only {
            "DO-EXTERNAL-SYMBOLS"
        } else {
            "DO-SYMBOLS"
        };
        if items.len() < 2 {
            return Err(Self::arity(operator, "at least one", 0));
        }
        let FormKind::List(binding) = &items[1].kind else {
            return Err(Self::invalid(
                "symbol iteration binding must be a list",
                items[1].span,
            ));
        };
        if !(1..=3).contains(&binding.len()) {
            return Err(Self::invalid(
                "symbol iteration binding must contain one to three forms",
                items[1].span,
            ));
        }
        if atom_name(&binding[0]).is_none() {
            return Err(Self::invalid(
                "symbol iteration variable must be a symbol",
                binding[0].span,
            ));
        }
        let package = binding
            .get(1)
            .cloned()
            .unwrap_or_else(|| Form::atom("NIL", form.span));
        let result = binding
            .get(2)
            .cloned()
            .unwrap_or_else(|| Form::atom("NIL", form.span));
        let symbols = Form::list(
            vec![
                Form::atom("__NCL-PACKAGE-SYMBOLS", form.span),
                package,
                Form::atom(if external_only { "T" } else { "NIL" }, form.span),
            ],
            form.span,
        );
        let mut dolist = vec![Form::atom("DOLIST", form.span), Form::list(vec![binding[0].clone(), symbols, result], items[1].span)];
        dolist.extend(items[2..].iter().cloned());
        Ok(Form::list(dolist, form.span))
    }
}

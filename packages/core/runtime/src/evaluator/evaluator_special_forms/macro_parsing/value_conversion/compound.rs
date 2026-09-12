use std::collections::HashSet;

use ncl_syntax::{Form, FormKind, Span};

use crate::{Runtime, RuntimeError, Value};

impl Runtime {
    pub(super) fn cons_form_from_value(
        value: &Value,
        span: Span,
        active: &mut HashSet<usize>,
    ) -> Result<Form, RuntimeError> {
        let mut current = value.clone();
        let mut cells = Vec::new();
        let mut forms = Vec::new();
        let converted = (|| {
            while let Value::Cons(cell) = current {
                if !active.insert(cell.identity()) {
                    return Ok(Form::new(FormKind::CircularReference, span));
                }
                cells.push(cell.clone());
                // A future CDR shared with this CAR is not yet an active ancestor.
                forms.push(Self::form_from_value_inner(&cell.car(), span, active)?);
                current = cell.cdr();
            }
            let proper = matches!(current, Value::Nil | Value::Boolean(false));
            if proper {
                Ok(Form::list(forms, span))
            } else {
                Ok(Form::dotted_list(
                    forms,
                    Self::form_from_value_inner(&current, span, active)?,
                    span,
                ))
            }
        })();
        for cell in cells {
            active.remove(&cell.identity());
        }
        converted
    }
}

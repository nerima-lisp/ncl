use super::{Form, FormKind};

pub(in crate::evaluator) fn atom_name(form: &Form) -> Option<&str> {
    match &form.kind {
        FormKind::Atom(value) => Some(value),
        _ => None,
    }
}

pub(in crate::evaluator) fn is_nil_form(form: &Form) -> bool {
    atom_name(form).is_some_and(|name| name.eq_ignore_ascii_case("nil"))
}

pub(in crate::evaluator) fn is_operator_form(form: &Form, name: &str) -> bool {
    match &form.kind {
        FormKind::List(items) => items
            .first()
            .and_then(atom_name)
            .is_some_and(|operator| operator.eq_ignore_ascii_case(name)),
        _ => false,
    }
}

pub(in crate::evaluator) fn prefix_argument<'form>(
    items: &'form [Form],
    name: &str,
) -> Option<&'form Form> {
    if items.len() != 2 {
        return None;
    }
    atom_name(&items[0]).filter(|operator| operator.eq_ignore_ascii_case(name))?;
    Some(&items[1])
}

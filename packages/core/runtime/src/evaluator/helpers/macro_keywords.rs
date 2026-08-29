use super::{Form, normalize_name};
use super::form_predicates::atom_name;

pub(in crate::evaluator) fn is_macro_keyword_form(form: &Form) -> bool {
    macro_keyword_name(form).is_some()
}

pub(in crate::evaluator) fn macro_keyword_name(form: &Form) -> Option<String> {
    let name = atom_name(form)?;
    let keyword = name.strip_prefix(':')?;
    (!keyword.is_empty()).then(|| normalize_name(keyword))
}

use crate::{CompileState, Form, FormKind};

pub(crate) fn generalized_list_place(form: &Form) -> Option<(Vec<String>, String, bool)> {
    let FormKind::List(items) = &form.kind else {
        return None;
    };
    if items.len() != 2 {
        return None;
    }
    let (accessor, _) = CompileState::symbol_name_info(&items[0], "list accessor").ok()?;
    if !matches!(accessor.as_str(), "CAR" | "FIRST" | "CDR" | "REST") {
        return None;
    }
    if let Some((mut accessors, name, escaped)) = generalized_list_place(&items[1]) {
        accessors.push(accessor);
        Some((accessors, name, escaped))
    } else {
        let (name, escaped) =
            CompileState::symbol_name_info(&items[1], "list place target").ok()?;
        Some((vec![accessor], name, escaped))
    }
}

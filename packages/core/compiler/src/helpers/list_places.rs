use crate::{CompileState, Constant, Form, FormKind};

pub(crate) fn list_accessor_target(form: &Form) -> Option<(String, &Form)> {
    let FormKind::List(items) = &form.kind else {
        return None;
    };
    if items.is_empty() {
        return None;
    }
    let (operator, _) = CompileState::symbol_name_info(&items[0], "list accessor").ok()?;
    if items.len() == 2
        && matches!(
            operator.as_str(),
            "CAR"
                | "FIRST"
                | "CDR"
                | "REST"
                | "SECOND"
                | "THIRD"
                | "FOURTH"
                | "FIFTH"
                | "SIXTH"
                | "SEVENTH"
                | "EIGHTH"
                | "NINTH"
                | "TENTH"
        )
    {
        return Some((operator, &items[1]));
    }
    if operator == "NTH" && items.len() == 3 {
        let FormKind::Atom(index) = &items[1].kind else {
            return None;
        };
        let Some(Constant::Integer(index)) = crate::helpers::literal_constant(index) else {
            return None;
        };
        let Ok(index) = usize::try_from(index) else {
            return None;
        };
        let accessor = match index {
            0 => "CAR",
            1 => "SECOND",
            2 => "THIRD",
            3 => "FOURTH",
            4 => "FIFTH",
            5 => "SIXTH",
            6 => "SEVENTH",
            7 => "EIGHTH",
            8 => "NINTH",
            9 => "TENTH",
            _ => return None,
        };
        return Some((accessor.to_owned(), &items[2]));
    }
    None
}

pub(crate) fn generalized_list_place(form: &Form) -> Option<(Vec<String>, String, bool)> {
    let (accessor, target) = list_accessor_target(form)?;
    if let Some((mut accessors, name, escaped)) = generalized_list_place(target) {
        accessors.push(accessor);
        Some((accessors, name, escaped))
    } else {
        let (name, escaped) = CompileState::symbol_name_info(target, "list place target").ok()?;
        Some((vec![accessor], name, escaped))
    }
}

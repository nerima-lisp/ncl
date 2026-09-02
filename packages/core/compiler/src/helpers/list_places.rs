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
        && (matches!(
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
        ) || is_composite_list_accessor(&operator))
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

pub(crate) fn is_composite_list_accessor(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() >= 4
        && bytes[0] == b'C'
        && bytes[bytes.len() - 1] == b'R'
        && bytes[1..bytes.len() - 1]
            .iter()
            .all(|byte| *byte == b'A' || *byte == b'D')
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

pub(crate) fn generalized_array_place(form: &Form) -> Option<(String, Vec<String>, String, bool)> {
    let FormKind::List(items) = &form.kind else {
        return None;
    };
    if items.len() < 3 {
        return None;
    }
    let (accessor, _) = CompileState::symbol_name_info(&items[0], "array place operator").ok()?;
    if !matches!(accessor.as_str(), "AREF" | "SVREF" | "ROW-MAJOR-AREF") {
        return None;
    }
    if let Some((accessors, name, escaped)) = generalized_list_place(&items[1]) {
        Some((accessor, accessors, name, escaped))
    } else {
        let (name, escaped) =
            CompileState::symbol_name_info(&items[1], "array place target").ok()?;
        Some((accessor, Vec::new(), name, escaped))
    }
}

pub(crate) fn dynamic_nth_list_place(
    form: &Form,
) -> Option<(&Form, &Form, Vec<String>, String, bool)> {
    let FormKind::List(items) = &form.kind else {
        return None;
    };
    if items.len() != 3
        || CompileState::symbol_name_info(&items[0], "list accessor")
            .ok()?
            .0
            != "NTH"
    {
        return None;
    }
    let mut accessors = Vec::new();
    let mut target = &items[2];
    while let Some((accessor, next_target)) = list_accessor_target(target) {
        accessors.push(accessor);
        target = next_target;
    }
    let (name, escaped) = CompileState::symbol_name_info(target, "list place target").ok()?;
    accessors.reverse();
    Some((&items[1], target, accessors, name, escaped))
}

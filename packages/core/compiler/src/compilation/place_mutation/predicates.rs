use super::*;

pub(super) fn stable_list_place(place: &Form) -> bool {
    let FormKind::List(items) = &place.kind else {
        return false;
    };
    if items.len() != 2 || !matches!(&items[1].kind, FormKind::Atom(_)) {
        return false;
    }
    CompileState::symbol_name_info(&items[0], "list place operator").is_ok_and(|(name, escaped)| {
        !escaped && matches!(name.as_str(), "CAR" | "CDR" | "FIRST" | "REST")
    })
}

pub(super) fn stable_nth_place(place: &Form) -> bool {
    let FormKind::List(items) = &place.kind else {
        return false;
    };
    items.len() == 3
        && CompileState::symbol_name_info(&items[0], "NTH place operator")
            .is_ok_and(|(name, escaped)| !escaped && name == "NTH")
        && matches!(items[2].kind, FormKind::Atom(_))
}

pub(super) fn stable_aref_vector_place(place: &Form) -> bool {
    let FormKind::List(items) = &place.kind else {
        return false;
    };
    items.len() == 3
        && CompileState::symbol_name_info(&items[0], "AREF place operator")
            .is_ok_and(|(name, escaped)| !escaped && name == "AREF")
        && matches!(items[1].kind, FormKind::Atom(_))
}

pub(super) fn stable_aref_array_place(place: &Form) -> bool {
    let FormKind::List(items) = &place.kind else {
        return false;
    };
    items.len() >= 4
        && CompileState::symbol_name_info(&items[0], "AREF place operator")
            .is_ok_and(|(name, escaped)| !escaped && name == "AREF")
        && matches!(items[1].kind, FormKind::Atom(_))
}

pub(super) fn stable_bit_place(place: &Form) -> bool {
    let FormKind::List(items) = &place.kind else {
        return false;
    };
    items.len() >= 3
        && CompileState::symbol_name_info(&items[0], "BIT place operator")
            .is_ok_and(|(name, escaped)| !escaped && name == "BIT")
        && matches!(items[1].kind, FormKind::Atom(_))
}

pub(super) fn stable_gethash_place(place: &Form) -> bool {
    let FormKind::List(items) = &place.kind else {
        return false;
    };
    items.len() == 3
        && CompileState::symbol_name_info(&items[0], "GETHASH place operator")
            .is_ok_and(|(name, escaped)| !escaped && name == "GETHASH")
}

pub(super) fn stable_getf_place(place: &Form) -> bool {
    let FormKind::List(items) = &place.kind else {
        return false;
    };
    items.len() == 3
        && CompileState::symbol_name_info(&items[0], "GETF place operator")
            .is_ok_and(|(name, escaped)| !escaped && name == "GETF")
}

pub(super) fn stable_row_major_aref_place(place: &Form) -> bool {
    let FormKind::List(items) = &place.kind else {
        return false;
    };
    items.len() == 3
        && CompileState::symbol_name_info(&items[0], "ROW-MAJOR-AREF place operator")
            .is_ok_and(|(name, escaped)| !escaped && name == "ROW-MAJOR-AREF")
        && matches!(items[1].kind, FormKind::Atom(_))
}

pub(super) fn stable_svref_place(place: &Form) -> bool {
    let FormKind::List(items) = &place.kind else {
        return false;
    };
    items.len() == 3
        && CompileState::symbol_name_info(&items[0], "SVREF place operator")
            .is_ok_and(|(name, escaped)| !escaped && name == "SVREF")
        && matches!(items[1].kind, FormKind::Atom(_))
}

pub(super) fn stable_string_char_place(place: &Form) -> bool {
    let FormKind::List(items) = &place.kind else {
        return false;
    };
    items.len() == 3
        && CompileState::symbol_name_info(&items[0], "CHAR place operator")
            .is_ok_and(|(name, escaped)| !escaped && matches!(name.as_str(), "CHAR" | "SCHAR"))
        && matches!(items[1].kind, FormKind::Atom(_))
}

pub(super) fn stable_elt_place(place: &Form) -> bool {
    let FormKind::List(items) = &place.kind else {
        return false;
    };
    items.len() == 3
        && CompileState::symbol_name_info(&items[0], "ELT place operator")
            .is_ok_and(|(name, escaped)| !escaped && name == "ELT")
        && matches!(items[1].kind, FormKind::Atom(_))
}

pub(super) fn stable_subseq_place(place: &Form) -> bool {
    let FormKind::List(items) = &place.kind else {
        return false;
    };
    (3..=4).contains(&items.len())
        && CompileState::symbol_name_info(&items[0], "SUBSEQ place operator")
            .is_ok_and(|(name, escaped)| !escaped && name == "SUBSEQ")
        && items[1..]
            .iter()
            .all(|item| matches!(item.kind, FormKind::Atom(_)))
}

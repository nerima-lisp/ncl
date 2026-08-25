use crate::environment::normalize_name;
use crate::value::ConditionSlot;
use crate::Environment;

pub(super) fn condition_precedence(
    name: &str,
    parents: &[String],
    environment: &Environment,
) -> Vec<String> {
    let mut sequences = parents
        .iter()
        .map(|parent| parent_precedence(parent, environment))
        .collect::<Vec<_>>();
    sequences.push(parents.to_vec());

    let mut precedence = vec![name.to_owned()];
    if let Some(merged) = merge_c3(&mut sequences) {
        for type_name in merged {
            add_unique(&mut precedence, &type_name);
        }
    } else {
        for parent in parents {
            for type_name in parent_precedence(parent, environment) {
                add_unique(&mut precedence, &type_name);
            }
        }
    }
    add_unique(&mut precedence, "CONDITION");
    precedence
}

fn parent_precedence(parent: &str, environment: &Environment) -> Vec<String> {
    if let Some(definition) = environment.lookup_condition(parent) {
        definition.precedence.clone()
    } else {
        let builtin = builtin_precedence(parent);
        if builtin.is_empty() {
            vec![parent.to_owned()]
        } else {
            builtin.iter().map(|name| (*name).to_owned()).collect()
        }
    }
}

fn merge_c3(sequences: &mut [Vec<String>]) -> Option<Vec<String>> {
    let mut merged = Vec::new();
    while sequences.iter().any(|sequence| !sequence.is_empty()) {
        let candidate = sequences
            .iter()
            .filter_map(|sequence| sequence.first())
            .find(|candidate| {
                !sequences.iter().any(|sequence| {
                    sequence
                        .iter()
                        .skip(1)
                        .any(|name| name.eq_ignore_ascii_case(candidate))
                })
            })?
            .clone();
        merged.push(candidate.clone());
        for sequence in sequences.iter_mut() {
            if sequence
                .first()
                .is_some_and(|name| name.eq_ignore_ascii_case(&candidate))
            {
                sequence.remove(0);
            }
        }
    }
    Some(merged)
}

fn builtin_precedence(name: &str) -> &'static [&'static str] {
    match normalize_name(name).as_str() {
        "CONDITION" => &["CONDITION"],
        "SERIOUS-CONDITION" => &["SERIOUS-CONDITION", "CONDITION"],
        "ERROR" => &["ERROR", "SERIOUS-CONDITION", "CONDITION"],
        "WARNING" => &["WARNING", "CONDITION"],
        "SIMPLE-CONDITION" => &["SIMPLE-CONDITION", "CONDITION"],
        "SIMPLE-ERROR" => &[
            "SIMPLE-ERROR",
            "ERROR",
            "SERIOUS-CONDITION",
            "SIMPLE-CONDITION",
            "CONDITION",
        ],
        "SIMPLE-TYPE-ERROR" => &[
            "SIMPLE-TYPE-ERROR",
            "TYPE-ERROR",
            "ERROR",
            "SERIOUS-CONDITION",
            "SIMPLE-CONDITION",
            "CONDITION",
        ],
        "SIMPLE-WARNING" => &["SIMPLE-WARNING", "WARNING", "SIMPLE-CONDITION", "CONDITION"],
        "TYPE-ERROR" => &["TYPE-ERROR", "ERROR", "SERIOUS-CONDITION", "CONDITION"],
        "PROGRAM-ERROR" => &["PROGRAM-ERROR", "ERROR", "SERIOUS-CONDITION", "CONDITION"],
        "PACKAGE-ERROR" => &["PACKAGE-ERROR", "ERROR", "SERIOUS-CONDITION", "CONDITION"],
        "READER-ERROR" => &["READER-ERROR", "ERROR", "SERIOUS-CONDITION", "CONDITION"],
        "COMPILER-ERROR" => &["COMPILER-ERROR", "ERROR", "SERIOUS-CONDITION", "CONDITION"],
        "FILE-ERROR" => &["FILE-ERROR", "ERROR", "SERIOUS-CONDITION", "CONDITION"],
        "UNBOUND-VARIABLE" => &[
            "UNBOUND-VARIABLE",
            "ERROR",
            "SERIOUS-CONDITION",
            "CONDITION",
        ],
        "CONTROL-ERROR" => &["CONTROL-ERROR", "ERROR", "SERIOUS-CONDITION", "CONDITION"],
        "ARITHMETIC-ERROR" => &[
            "ARITHMETIC-ERROR",
            "ERROR",
            "SERIOUS-CONDITION",
            "CONDITION",
        ],
        "DIVISION-BY-ZERO" => &[
            "DIVISION-BY-ZERO",
            "ARITHMETIC-ERROR",
            "ERROR",
            "SERIOUS-CONDITION",
            "CONDITION",
        ],
        _ => &[],
    }
}

pub(super) fn inherited_slots(
    precedence: &[String],
    own_slots: &[ConditionSlot],
    environment: &Environment,
) -> Vec<ConditionSlot> {
    let mut slots = Vec::new();
    for parent in precedence.iter().skip(1).rev() {
        if let Some(definition) = environment.lookup_condition(parent) {
            for slot in &definition.slots {
                replace_slot(&mut slots, slot.clone());
            }
        }
    }
    for slot in own_slots {
        replace_slot(&mut slots, slot.clone());
    }
    slots
}

fn replace_slot(slots: &mut Vec<ConditionSlot>, slot: ConditionSlot) {
    if let Some(index) = slots
        .iter()
        .position(|existing| normalize_name(&existing.name) == normalize_name(&slot.name))
    {
        slots[index] = slot;
    } else {
        slots.push(slot);
    }
}

fn add_unique(names: &mut Vec<String>, name: &str) {
    if !names
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(name))
    {
        names.push(name.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::merge_c3;

    fn sequence(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn merges_diamond_condition_precedence() {
        let mut sequences = vec![
            sequence(&["LEFT", "ROOT", "CONDITION"]),
            sequence(&["RIGHT", "ROOT", "CONDITION"]),
            sequence(&["LEFT", "RIGHT"]),
        ];
        assert_eq!(
            merge_c3(&mut sequences),
            Some(sequence(&["LEFT", "RIGHT", "ROOT", "CONDITION"]))
        );
    }

    #[test]
    fn rejects_inconsistent_condition_precedence() {
        let mut sequences = vec![
            sequence(&["A", "B"]),
            sequence(&["B", "A"]),
            sequence(&["A", "B"]),
        ];
        assert_eq!(merge_c3(&mut sequences), None);
    }
}

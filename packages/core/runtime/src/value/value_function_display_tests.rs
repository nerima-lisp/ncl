#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::super::{Environment, Function, Value};

    #[test]
    fn function_display_forms_cover_generated_function_kinds() {
        let cases = [
            (
                Function::StructureConstructor {
                    name: "MAKE-POINT".to_string(),
                    slots: Vec::new(),
                    structure_types: Vec::new(),
                    constructor_lambda_list: None,
                    environment: Environment::new(),
                },
                "#<STRUCTURE-CONSTRUCTOR MAKE-POINT>",
            ),
            (
                Function::StructurePredicate {
                    name: "POINT-P".to_string(),
                },
                "#<STRUCTURE-PREDICATE POINT-P>",
            ),
            (
                Function::StructureAccessor {
                    structure_name: "POINT".to_string(),
                    slot_name: "X".to_string(),
                    slot_index: 0,
                    read_only: false,
                },
                "#<STRUCTURE-ACCESSOR POINT-X>",
            ),
            (
                Function::StructureCopier {
                    name: "COPY-POINT".to_string(),
                },
                "#<STRUCTURE-COPIER COPY-POINT>",
            ),
            (
                Function::ConditionReader {
                    condition_name: "ERROR".to_string(),
                    slot_name: "MESSAGE".to_string(),
                },
                "#<CONDITION-READER ERROR-MESSAGE>",
            ),
            (
                Function::ConditionWriter {
                    condition_name: "ERROR".to_string(),
                    slot_name: "MESSAGE".to_string(),
                },
                "#<CONDITION-WRITER ERROR-MESSAGE>",
            ),
        ];

        for (function, expected) in cases {
            assert_eq!(Value::Function(Rc::new(function)).to_string(), expected);
        }
    }
}
